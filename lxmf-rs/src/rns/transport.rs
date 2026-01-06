use crate::constants::{
    DESTINATION_LENGTH, METHOD_DIRECT, METHOD_OPPORTUNISTIC, METHOD_PAPER, METHOD_PROPAGATED,
    REPR_PACKET, REPR_RESOURCE,
};
use crate::error::LXMFError;
use crate::message::LXMessage;
use rand_core::OsRng;
use reticulum::destination::{DestinationName, SingleInputDestination, SingleOutputDestination};
use reticulum::destination::link::{LinkEvent, LinkEventData};
use reticulum::identity::{EncryptIdentity, Identity, PrivateIdentity};
use reticulum::iface::udp::UdpInterface;
use reticulum::packet::{
    DestinationType, Header, Packet, PacketContext, PacketDataBuffer, PacketType, PACKET_MDU,
};
use reticulum::transport::{Transport, TransportConfig};
use rmpv::Value;
use sha2::Digest;
use std::io::Cursor;
use tokio::sync::broadcast;

use super::pn::{PnDirRequest, PnDirResponse};
use super::{full_hash, value_to_f64, value_to_fixed_bytes, DeliveryMode, RnsCryptoPolicy, RnsError};
use super::router::{CONTROL_STATS_PATH, CONTROL_SYNC_PATH, CONTROL_UNPEER_PATH};

#[derive(Debug, PartialEq, Eq)]
pub enum DeliveryOutput {
    Packet(Packet),
    Resource {
        destination: reticulum::hash::AddressHash,
        context: PacketContext,
        data: Vec<u8>,
    },
    PaperUri(String),
}

#[derive(Debug, PartialEq)]
pub enum Received {
    Message(LXMessage),
    Propagated {
        timestamp: f64,
        lxm_data: Vec<u8>,
        transient_id: [u8; 32],
        source_hash: Option<[u8; DESTINATION_LENGTH]>,
        count_client_receive: bool,
    },
    PropagationResource {
        payload: Vec<u8>,
        source_hash: Option<[u8; DESTINATION_LENGTH]>,
    },
    Request {
        request: RnsRequest,
        source_hash: Option<[u8; DESTINATION_LENGTH]>,
    },
    Response {
        response: RnsResponse,
        source_hash: Option<[u8; DESTINATION_LENGTH]>,
    },
    ControlRequest {
        request: RnsRequest,
        source_hash: Option<[u8; DESTINATION_LENGTH]>,
    },
}

pub trait EncryptHook: Send + Sync {
    fn encrypt(&self, destination: &SingleOutputDestination, plaintext: &[u8]) -> Result<Vec<u8>, RnsError>;

    fn provides_encryption(&self) -> bool {
        true
    }
}

pub struct ReticulumEncryptor;

impl EncryptHook for ReticulumEncryptor {
    fn encrypt(&self, destination: &SingleOutputDestination, plaintext: &[u8]) -> Result<Vec<u8>, RnsError> {
        let salt = Some(destination.desc.address_hash.as_slice());
        let derived = destination.identity.derive_key(OsRng, salt);
        let mut out_buf = vec![0u8; plaintext.len() + 512];
        let encrypted = destination
            .identity
            .encrypt(OsRng, plaintext, &derived, &mut out_buf)?;
        Ok(encrypted.to_vec())
    }
}

pub struct NullEncryptor;

impl EncryptHook for NullEncryptor {
    fn encrypt(&self, _destination: &SingleOutputDestination, plaintext: &[u8]) -> Result<Vec<u8>, RnsError> {
        Ok(plaintext.to_vec())
    }

    fn provides_encryption(&self) -> bool {
        false
    }
}

pub struct RnsOutbound {
    pub source: SingleInputDestination,
    pub destination: SingleOutputDestination,
    encryptor: Box<dyn EncryptHook>,
    crypto_policy: RnsCryptoPolicy,
}

pub struct RnsNodeRouter {
    pub identity: PrivateIdentity,
    pub transport: Transport,
    pub destination: std::sync::Arc<tokio::sync::Mutex<SingleInputDestination>>,
    in_events: broadcast::Receiver<LinkEventData>,
    out_events: broadcast::Receiver<LinkEventData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RnsRequest {
    pub request_id: [u8; DESTINATION_LENGTH],
    pub requested_at: f64,
    pub path_hash: [u8; DESTINATION_LENGTH],
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RnsResponse {
    pub request_id: [u8; DESTINATION_LENGTH],
    pub data: Value,
}

impl RnsOutbound {
    pub fn new(source: PrivateIdentity, destination: Identity) -> Self {
        let name = DestinationName::new("lxmf", "delivery");
        let source_dest = SingleInputDestination::new(source, name);
        let dest = SingleOutputDestination::new(destination, name);

        Self {
            source: source_dest,
            destination: dest,
            encryptor: Box::new(ReticulumEncryptor),
            crypto_policy: RnsCryptoPolicy::default(),
        }
    }

    pub fn with_encryptor(
        source: PrivateIdentity,
        destination: Identity,
        encryptor: Box<dyn EncryptHook>,
    ) -> Self {
        let name = DestinationName::new("lxmf", "delivery");
        let source_dest = SingleInputDestination::new(source, name);
        let dest = SingleOutputDestination::new(destination, name);

        Self {
            source: source_dest,
            destination: dest,
            encryptor,
            crypto_policy: RnsCryptoPolicy::default(),
        }
    }

    pub fn crypto_policy(&self) -> RnsCryptoPolicy {
        self.crypto_policy
    }

    pub fn set_crypto_policy(&mut self, policy: RnsCryptoPolicy) {
        self.crypto_policy = policy;
    }

    pub fn with_crypto_policy(mut self, policy: RnsCryptoPolicy) -> Self {
        self.crypto_policy = policy;
        self
    }

    pub fn prepare_message(&self, msg: &mut LXMessage) -> Result<(), RnsError> {
        msg.destination_hash = address_hash_to_array(&self.destination.desc.address_hash)?;
        msg.source_hash = address_hash_to_array(&self.source.desc.address_hash)?;
        Ok(())
    }

    pub fn deliver(&self, msg: &mut LXMessage, mode: DeliveryMode) -> Result<DeliveryOutput, RnsError> {
        self.crypto_policy.check_outbound(mode, self.encryptor.as_ref())?;
        self.prepare_message(msg)?;

        let packed = msg.encode_signed(self.source.identity.sign_key())?;

        match mode {
            DeliveryMode::Opportunistic => {
                msg.method = Some(METHOD_OPPORTUNISTIC);
                let payload = &packed[DESTINATION_LENGTH..];
                deliver_as_packet_or_resource(&self.destination, payload, msg, DestinationType::Single)
            }
            DeliveryMode::Direct => {
                msg.method = Some(METHOD_DIRECT);
                deliver_as_packet_or_resource(&self.destination, &packed, msg, DestinationType::Single)
            }
            DeliveryMode::Propagated => {
                msg.method = Some(METHOD_PROPAGATED);
                let encrypted = self.encryptor.encrypt(&self.destination, &packed[DESTINATION_LENGTH..])?;
                let propagation = msg.propagation_packed_from_encrypted(&encrypted, msg.timestamp, None)?;
                deliver_as_packet_or_resource(&self.destination, &propagation, msg, DestinationType::Single)
            }
            DeliveryMode::Paper => {
                msg.method = Some(METHOD_PAPER);
                let encrypted = self.encryptor.encrypt(&self.destination, &packed[DESTINATION_LENGTH..])?;
                let paper = msg.paper_packed_from_encrypted(&encrypted);
                let uri = msg.as_uri_from_paper(&paper);
                Ok(DeliveryOutput::PaperUri(uri))
            }
        }
    }
}

impl RnsCryptoPolicy {
    pub(crate) fn check_outbound(&self, mode: DeliveryMode, encryptor: &dyn EncryptHook) -> Result<(), RnsError> {
        match self {
            RnsCryptoPolicy::AllowPlaintext => Ok(()),
            RnsCryptoPolicy::EnforceRatchets => match mode {
                DeliveryMode::Opportunistic | DeliveryMode::Direct => Err(RnsError::CryptoPolicyViolation {
                    policy: *self,
                    mode,
                    reason: "delivery mode does not apply LXMF-layer encryption yet".to_string(),
                }),
                DeliveryMode::Propagated | DeliveryMode::Paper => {
                    if encryptor.provides_encryption() {
                        Ok(())
                    } else {
                        Err(RnsError::CryptoPolicyViolation {
                            policy: *self,
                            mode,
                            reason: "encryptor does not provide encryption (NullEncryptor?)".to_string(),
                        })
                    }
                }
            },
        }
    }
}

impl RnsNodeRouter {
    pub async fn new_udp<T: Into<String>>(
        name: T,
        identity: PrivateIdentity,
        bind_addr: &str,
        forward_addr: &str,
    ) -> Result<Self, RnsError> {
        let config = TransportConfig::new(name, &identity, true);
        let mut transport = Transport::new(config);

        transport
            .iface_manager()
            .lock()
            .await
            .spawn(UdpInterface::new(bind_addr, Some(forward_addr)), UdpInterface::spawn);

        let destination = transport
            .add_destination(identity.clone(), DestinationName::new("lxmf", "delivery"))
            .await;

        let in_events = transport.in_link_events();
        let out_events = transport.out_link_events();

        Ok(Self {
            identity,
            transport,
            destination,
            in_events,
            out_events,
        })
    }

    pub async fn send_message(
        &self,
        destination: Identity,
        msg: &mut LXMessage,
        mode: DeliveryMode,
    ) -> Result<(), RnsError> {
        let outbound = RnsOutbound::new(self.identity.clone(), destination);
        let delivery = outbound.deliver(msg, mode)?;

        match delivery {
            DeliveryOutput::Packet(packet) => {
                self.transport.send_packet(packet).await;
            }
            DeliveryOutput::Resource { destination, data, .. } => {
                self.transport.send_to_out_links(&destination, &data).await;
            }
            DeliveryOutput::PaperUri(_) => {}
        }

        Ok(())
    }

    pub async fn announce(&self) {
        self.transport.send_announce(&self.destination, None).await;
    }

pub async fn poll_inbound(&mut self) -> Vec<Received> {
    let mut out = Vec::new();
    while let Ok(event) = self.in_events.try_recv() {
        let source_hash = source_hash_from_event(&event);
        let count_client_receive = source_hash.is_none();
        if let LinkEvent::Data(payload) = &event.event {
            if let Ok(msg) = LXMessage::decode(payload.as_slice()) {
                out.push(Received::Message(msg));
            } else if let Ok(received) = decode_propagated_payload(
                payload.as_slice(),
                source_hash,
                count_client_receive,
            ) {
                out.push(received);
            } else if decode_propagation_resource_payload(payload.as_slice()).is_ok() {
                out.push(Received::PropagationResource {
                    payload: payload.as_slice().to_vec(),
                    source_hash,
                });
            } else if let Ok(request) = address_hash_to_array(&event.address_hash)
                .and_then(|destination_hash| {
                    decode_request_bytes_for_destination(destination_hash, payload.as_slice())
                })
            {
                if let Some(path) = request_path_for_hash(request.path_hash) {
                    if matches!(path, CONTROL_STATS_PATH | CONTROL_SYNC_PATH | CONTROL_UNPEER_PATH)
                    {
                        out.push(Received::ControlRequest {
                            request,
                            source_hash,
                        });
                    } else {
                        out.push(Received::Request {
                            request,
                            source_hash,
                        });
                    }
                } else {
                    out.push(Received::Request {
                        request,
                        source_hash,
                    });
                }
            } else if let Ok(response) = decode_response_bytes(payload.as_slice()) {
                out.push(Received::Response {
                    response,
                    source_hash,
                });
            }
        }
    }
    while let Ok(event) = self.out_events.try_recv() {
        let source_hash = source_hash_from_event(&event);
        let count_client_receive = source_hash.is_none();
        if let LinkEvent::Data(payload) = &event.event {
            if let Ok(msg) = LXMessage::decode(payload.as_slice()) {
                out.push(Received::Message(msg));
            } else if let Ok(received) = decode_propagated_payload(
                payload.as_slice(),
                source_hash,
                count_client_receive,
            ) {
                out.push(received);
            } else if decode_propagation_resource_payload(payload.as_slice()).is_ok() {
                out.push(Received::PropagationResource {
                    payload: payload.as_slice().to_vec(),
                    source_hash,
                });
            } else if let Ok(request) = address_hash_to_array(&event.address_hash)
                .and_then(|destination_hash| {
                    decode_request_bytes_for_destination(destination_hash, payload.as_slice())
                })
            {
                if let Some(path) = request_path_for_hash(request.path_hash) {
                    if matches!(path, CONTROL_STATS_PATH | CONTROL_SYNC_PATH | CONTROL_UNPEER_PATH)
                    {
                        out.push(Received::ControlRequest {
                            request,
                            source_hash,
                        });
                    } else {
                        out.push(Received::Request {
                            request,
                            source_hash,
                        });
                    }
                } else {
                    out.push(Received::Request {
                        request,
                        source_hash,
                    });
                }
            } else if let Ok(response) = decode_response_bytes(payload.as_slice()) {
                out.push(Received::Response {
                    response,
                    source_hash,
                });
            }
        }
    }
    out
}

    pub fn build_pn_dir_request_packet(
        &self,
        destination: Identity,
        req: &PnDirRequest,
    ) -> Result<Packet, RnsError> {
        let payload = crate::rns::encode_pn_dir_request(req)?;
        let dest = SingleOutputDestination::new(destination, DestinationName::new("lxmf", "pn"));
        packet_from_payload_with_context(
            &dest,
            &payload,
            DestinationType::Single,
            PacketContext::Request,
        )
    }

    pub fn build_pn_dir_response_packet(
        &self,
        destination: Identity,
        resp: &PnDirResponse,
    ) -> Result<Packet, RnsError> {
        let payload = crate::rns::encode_pn_dir_response(resp)?;
        let dest = SingleOutputDestination::new(destination, DestinationName::new("lxmf", "pn"));
        packet_from_payload_with_context(
            &dest,
            &payload,
            DestinationType::Single,
            PacketContext::Response,
        )
    }

    pub async fn send_pn_dir_request(
        &self,
        destination: Identity,
        req: &PnDirRequest,
    ) -> Result<(), RnsError> {
        let packet = self.build_pn_dir_request_packet(destination, req)?;
        self.transport.send_packet(packet).await;
        Ok(())
    }

    pub async fn send_pn_dir_response(
        &self,
        destination: Identity,
        resp: &PnDirResponse,
    ) -> Result<(), RnsError> {
        let packet = self.build_pn_dir_response_packet(destination, resp)?;
        self.transport.send_packet(packet).await;
        Ok(())
    }

    pub fn build_request_packet(
        &self,
        destination: Identity,
        req: &mut RnsRequest,
    ) -> Result<Packet, RnsError> {
        let payload = encode_request_bytes(req)?;
        let dest = SingleOutputDestination::new(destination, DestinationName::new("lxmf", "req"));
        let destination_hash = address_hash_to_array(&dest.desc.address_hash)?;
        let packet = packet_from_payload_with_context(
            &dest,
            &payload,
            DestinationType::Single,
            PacketContext::Request,
        )?;
        req.request_id = request_id_for_destination(destination_hash, &payload)?;
        Ok(packet)
    }

    pub fn build_response_packet(
        &self,
        destination: Identity,
        resp: &RnsResponse,
    ) -> Result<Packet, RnsError> {
        let payload = encode_response_bytes(resp)?;
        let dest = SingleOutputDestination::new(destination, DestinationName::new("lxmf", "req"));
        packet_from_payload_with_context(
            &dest,
            &payload,
            DestinationType::Single,
            PacketContext::Response,
        )
    }

    pub async fn send_request(
        &self,
        destination: Identity,
        req: &mut RnsRequest,
    ) -> Result<(), RnsError> {
        let packet = self.build_request_packet(destination, req)?;
        self.transport.send_packet(packet).await;
        Ok(())
    }

    pub async fn send_response(
        &self,
        destination: Identity,
        resp: &RnsResponse,
    ) -> Result<(), RnsError> {
        let packet = self.build_response_packet(destination, resp)?;
        self.transport.send_packet(packet).await;
        Ok(())
    }

    pub fn build_request_packet_for_hash(
        &self,
        destination_hash: [u8; DESTINATION_LENGTH],
        req: &mut RnsRequest,
    ) -> Result<Packet, RnsError> {
        let payload = encode_request_bytes(req)?;
        let packet = packet_from_payload_hash_with_context(
            destination_hash,
            &payload,
            DestinationType::Single,
            PacketContext::Request,
        )?;
        req.request_id = request_id_for_destination(destination_hash, &payload)?;
        Ok(packet)
    }

    pub fn build_response_packet_for_hash(
        &self,
        destination_hash: [u8; DESTINATION_LENGTH],
        resp: &RnsResponse,
    ) -> Result<Packet, RnsError> {
        let payload = encode_response_bytes(resp)?;
        packet_from_payload_hash_with_context(
            destination_hash,
            &payload,
            DestinationType::Single,
            PacketContext::Response,
        )
    }

    pub async fn send_request_to_hash(
        &self,
        destination_hash: [u8; DESTINATION_LENGTH],
        req: &mut RnsRequest,
    ) -> Result<(), RnsError> {
        let packet = self.build_request_packet_for_hash(destination_hash, req)?;
        self.transport.send_packet(packet).await;
        Ok(())
    }

    pub async fn send_response_to_hash(
        &self,
        destination_hash: [u8; DESTINATION_LENGTH],
        resp: &RnsResponse,
    ) -> Result<(), RnsError> {
        let packet = self.build_response_packet_for_hash(destination_hash, resp)?;
        self.transport.send_packet(packet).await;
        Ok(())
    }
}

pub fn decode_packet(packet: &Packet, mode: DeliveryMode) -> Result<Received, RnsError> {
    decode_packet_with_source(packet, mode, None)
}

pub fn decode_packet_with_source(
    packet: &Packet,
    mode: DeliveryMode,
    source_hash: Option<[u8; DESTINATION_LENGTH]>,
) -> Result<Received, RnsError> {
    match mode {
        DeliveryMode::Opportunistic => {
            let mut buf = Vec::with_capacity(DESTINATION_LENGTH + packet.data.len());
            buf.extend_from_slice(packet.destination.as_slice());
            buf.extend_from_slice(packet.data.as_slice());
            let msg = LXMessage::decode(&buf)?;
            Ok(Received::Message(msg))
        }
        DeliveryMode::Direct => {
            let msg = LXMessage::decode(packet.data.as_slice())?;
            Ok(Received::Message(msg))
        }
        DeliveryMode::Propagated => {
            decode_propagated_payload(packet.data.as_slice(), source_hash, true)
        }
        DeliveryMode::Paper => Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
    }
}

pub fn decode_propagation_resource_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(payload))?;
    let arr = match value {
        Value::Array(a) => a,
        _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
    };
    if arr.len() < 2 {
        return Err(RnsError::Lxmf(LXMFError::InvalidPayload));
    }
    let messages = match &arr[1] {
        Value::Array(items) => items,
        _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
    };
    let mut out = Vec::with_capacity(messages.len());
    for item in messages {
        match item {
            Value::Binary(bytes) => out.push(bytes.clone()),
            _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
        }
    }
    Ok(out)
}

fn decode_propagated_payload(
    payload: &[u8],
    source_hash: Option<[u8; DESTINATION_LENGTH]>,
    count_client_receive: bool,
) -> Result<Received, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(payload))?;
    let arr = match value {
        Value::Array(a) => a,
        _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
    };
    if arr.len() < 2 {
        return Err(RnsError::Lxmf(LXMFError::InvalidPayload));
    }
    let timestamp = match &arr[0] {
        Value::F64(v) => *v,
        _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
    };
    let lxm_data = match &arr[1] {
        Value::Array(inner) => {
            if inner.len() != 1 {
                return Err(RnsError::Lxmf(LXMFError::InvalidPayload));
            }
            match &inner[0] {
                Value::Binary(b) => b.clone(),
                _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
            }
        }
        _ => return Err(RnsError::Lxmf(LXMFError::InvalidPayload)),
    };
    let transient_id = full_hash(&lxm_data);
    Ok(Received::Propagated {
        timestamp,
        lxm_data,
        transient_id,
        source_hash,
        count_client_receive,
    })
}

fn source_hash_from_event(event: &LinkEventData) -> Option<[u8; DESTINATION_LENGTH]> {
    let hash = address_hash_to_array(&event.peer_identity_hash()?).ok()?;
    if hash.iter().all(|byte| *byte == 0) {
        None
    } else {
        Some(hash)
    }
}

pub fn encode_request_bytes(req: &RnsRequest) -> Result<Vec<u8>, RnsError> {
    let value = Value::Array(vec![
        Value::F64(req.requested_at),
        Value::Binary(req.path_hash.to_vec()),
        req.data.clone(),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub fn decode_request_bytes(bytes: &[u8]) -> Result<RnsRequest, RnsError> {
    decode_request_bytes_with_request_id(bytes, request_id_from_payload(bytes))
}

pub fn decode_request_bytes_for_destination(
    destination_hash: [u8; DESTINATION_LENGTH],
    bytes: &[u8],
) -> Result<RnsRequest, RnsError> {
    let request_id = request_id_for_destination(destination_hash, bytes)?;
    decode_request_bytes_with_request_id(bytes, request_id)
}

fn decode_request_bytes_with_request_id(
    bytes: &[u8],
    request_id: [u8; DESTINATION_LENGTH],
) -> Result<RnsRequest, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("request must be array".to_string())),
    };
    if arr.len() < 3 {
        return Err(RnsError::Msgpack("request missing fields".to_string()));
    }
    let requested_at = value_to_f64(&arr[0])
        .ok_or_else(|| RnsError::Msgpack("invalid requested_at".to_string()))?;
    let path_hash = value_to_fixed_bytes::<DESTINATION_LENGTH>(&arr[1])
        .ok_or_else(|| RnsError::Msgpack("invalid request path hash".to_string()))?;
    let data = arr[2].clone();
    Ok(RnsRequest {
        request_id,
        requested_at,
        path_hash,
        data,
    })
}

pub fn encode_response_bytes(resp: &RnsResponse) -> Result<Vec<u8>, RnsError> {
    let value = Value::Array(vec![
        Value::Binary(resp.request_id.to_vec()),
        resp.data.clone(),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub fn decode_response_bytes(bytes: &[u8]) -> Result<RnsResponse, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("response must be array".to_string())),
    };
    if arr.len() < 2 {
        return Err(RnsError::Msgpack("response missing fields".to_string()));
    }
    let request_id = value_to_fixed_bytes::<DESTINATION_LENGTH>(&arr[0])
        .ok_or_else(|| RnsError::Msgpack("invalid response id".to_string()))?;
    let data = arr[1].clone();
    Ok(RnsResponse { request_id, data })
}

pub fn decode_request_packet(packet: &Packet) -> Result<RnsRequest, RnsError> {
    if packet.context != PacketContext::Request {
        return Err(RnsError::Msgpack("packet is not request".to_string()));
    }
    let mut req = decode_request_bytes(packet.data.as_slice())?;
    req.request_id = request_id_from_packet(packet);
    Ok(req)
}

pub fn decode_response_packet(packet: &Packet) -> Result<RnsResponse, RnsError> {
    if packet.context != PacketContext::Response {
        return Err(RnsError::Msgpack("packet is not response".to_string()));
    }
    decode_response_bytes(packet.data.as_slice())
}

pub(crate) fn request_path_hash(path: &str) -> [u8; DESTINATION_LENGTH] {
    truncate_sha256_bytes(path.as_bytes())
}

pub(crate) fn request_path_for_hash(hash: [u8; DESTINATION_LENGTH]) -> Option<&'static str> {
    if hash == request_path_hash(CONTROL_STATS_PATH) {
        Some(CONTROL_STATS_PATH)
    } else if hash == request_path_hash(CONTROL_SYNC_PATH) {
        Some(CONTROL_SYNC_PATH)
    } else if hash == request_path_hash(CONTROL_UNPEER_PATH) {
        Some(CONTROL_UNPEER_PATH)
    } else {
        None
    }
}

pub(crate) fn request_id_from_payload(payload: &[u8]) -> [u8; DESTINATION_LENGTH] {
    truncate_sha256_bytes(payload)
}

fn request_id_from_packet(packet: &Packet) -> [u8; DESTINATION_LENGTH] {
    let hash = packet.hash();
    let mut out = [0u8; DESTINATION_LENGTH];
    out.copy_from_slice(&hash.as_slice()[..DESTINATION_LENGTH]);
    out
}

pub(crate) fn request_id_for_destination(
    destination_hash: [u8; DESTINATION_LENGTH],
    payload: &[u8],
) -> Result<[u8; DESTINATION_LENGTH], RnsError> {
    Ok(request_id_from_packet_parts(
        destination_hash,
        PacketContext::Request,
        DestinationType::Single,
        PacketType::Data,
        payload,
    ))
}

fn request_id_from_packet_parts(
    destination_hash: [u8; DESTINATION_LENGTH],
    context: PacketContext,
    destination_type: DestinationType,
    packet_type: PacketType,
    payload: &[u8],
) -> [u8; DESTINATION_LENGTH] {
    let header = Header {
        destination_type,
        packet_type,
        ..Default::default()
    };
    let mut hasher = sha2::Sha256::new();
    hasher.update(&[header.to_meta() & 0b0000_1111]);
    hasher.update(&destination_hash);
    hasher.update(&[context as u8]);
    hasher.update(payload);
    let digest = hasher.finalize();
    let mut out = [0u8; DESTINATION_LENGTH];
    out.copy_from_slice(&digest[..DESTINATION_LENGTH]);
    out
}

fn truncate_sha256_bytes(input: &[u8]) -> [u8; DESTINATION_LENGTH] {
    let digest = sha2::Sha256::new().chain_update(input).finalize();
    let mut out = [0u8; DESTINATION_LENGTH];
    out.copy_from_slice(&digest[..DESTINATION_LENGTH]);
    out
}

fn deliver_as_packet_or_resource(
    destination: &SingleOutputDestination,
    payload: &[u8],
    msg: &mut LXMessage,
    destination_type: DestinationType,
) -> Result<DeliveryOutput, RnsError> {
    if payload.len() <= PACKET_MDU {
        msg.representation = REPR_PACKET;
        let packet = packet_from_payload(destination, payload, destination_type)?;
        Ok(DeliveryOutput::Packet(packet))
    } else {
        msg.representation = REPR_RESOURCE;
        Ok(DeliveryOutput::Resource {
            destination: destination.desc.address_hash,
            context: PacketContext::Resource,
            data: payload.to_vec(),
        })
    }
}

fn packet_from_payload(
    destination: &SingleOutputDestination,
    payload: &[u8],
    destination_type: DestinationType,
) -> Result<Packet, RnsError> {
    let mut data = PacketDataBuffer::new();
    data.write(payload)?;

    Ok(Packet {
        header: Header {
            destination_type,
            packet_type: PacketType::Data,
            ..Default::default()
        },
        ifac: None,
        destination: destination.desc.address_hash,
        transport: None,
        context: PacketContext::None,
        data,
    })
}

fn packet_from_payload_with_context(
    destination: &SingleOutputDestination,
    payload: &[u8],
    destination_type: DestinationType,
    context: PacketContext,
) -> Result<Packet, RnsError> {
    let mut data = PacketDataBuffer::new();
    data.write(payload)?;

    Ok(Packet {
        header: Header {
            destination_type,
            packet_type: PacketType::Data,
            ..Default::default()
        },
        ifac: None,
        destination: destination.desc.address_hash,
        transport: None,
        context,
        data,
    })
}

fn packet_from_payload_hash_with_context(
    destination_hash: [u8; DESTINATION_LENGTH],
    payload: &[u8],
    destination_type: DestinationType,
    context: PacketContext,
) -> Result<Packet, RnsError> {
    let mut data = PacketDataBuffer::new();
    data.write(payload)?;

    Ok(Packet {
        header: Header {
            destination_type,
            packet_type: PacketType::Data,
            ..Default::default()
        },
        ifac: None,
        destination: reticulum::hash::AddressHash::new(destination_hash),
        transport: None,
        context,
        data,
    })
}

fn address_hash_to_array(hash: &reticulum::hash::AddressHash) -> Result<[u8; DESTINATION_LENGTH], RnsError> {
    let bytes = hash.as_slice();
    if bytes.len() != DESTINATION_LENGTH {
        return Err(RnsError::InvalidDestinationHash);
    }
    let mut out = [0u8; DESTINATION_LENGTH];
    out.copy_from_slice(bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reticulum::destination::link::{LinkEvent, LinkEventData, LinkPayload};
    use reticulum::hash::AddressHash;
    struct FailingEncryptor;

    impl EncryptHook for FailingEncryptor {
        fn encrypt(
            &self,
            _destination: &SingleOutputDestination,
            _plaintext: &[u8],
        ) -> Result<Vec<u8>, RnsError> {
            Err(RnsError::InvalidDestinationHash)
        }
    }

    fn make_outbound_failing() -> RnsOutbound {
        let source = PrivateIdentity::new_from_name("source-fail");
        let destination = PrivateIdentity::new_from_name("dest-fail").as_identity().clone();
        RnsOutbound::with_encryptor(source, destination, Box::new(FailingEncryptor))
    }

    #[test]
    fn deliver_propagated_fails_with_encryptor() {
        let outbound = make_outbound_failing();
        let mut msg = LXMessage::with_strings(
            [0u8; 16],
            [0u8; 16],
            1_700_000_000.0,
            "hi",
            "hello",
            Value::Map(vec![]),
        );
        let err = outbound.deliver(&mut msg, DeliveryMode::Propagated).unwrap_err();
        match err {
            RnsError::InvalidDestinationHash => {}
            _ => panic!("unexpected error"),
        }
    }

    #[test]
    fn zero_hash_source_is_none_and_skips_client_count() {
        let payload = vec![
            0x92, // array len 2
            0xcb, 0x3f, 0xf4, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // f64 1.25
            0x91, // inner array len 1
            0xc4, 0x11, // bin 17
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11,
        ];
        let event = LinkEventData::new(
            AddressHash::new_empty(),
            AddressHash::new_empty(),
            LinkEvent::Data(LinkPayload::new_from_slice(&payload)),
        );
        let source_hash = source_hash_from_event(&event);
        assert!(source_hash.is_none());

        let received = decode_propagated_payload(&payload, source_hash, false).expect("decoded");
        match received {
            Received::Propagated {
                count_client_receive,
                source_hash,
                ..
            } => {
                assert!(!count_client_receive);
                assert!(source_hash.is_none());
            }
            _ => panic!("expected propagated"),
        }
    }

    #[test]
    fn poll_inbound_falls_back_to_resource_payload() {
        let payload = Value::Array(vec![
            Value::F64(1_700_000_000.0),
            Value::Array(vec![
                Value::Binary(vec![0x01u8; 32]),
                Value::Binary(vec![0x02u8; 32]),
            ]),
        ]);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &payload).expect("msgpack");

        let event = LinkEventData::new(
            AddressHash::new_empty(),
            AddressHash::new_empty(),
            LinkEvent::Data(LinkPayload::new_from_slice(&buf)),
        );

        let source_hash = source_hash_from_event(&event);
        assert!(source_hash.is_none());
        let received = decode_propagated_payload(&buf, source_hash, true);
        assert!(received.is_err());

        let resource = decode_propagation_resource_payload(&buf).expect("resource");
        assert_eq!(resource.len(), 2);
    }

    #[test]
    fn request_response_payload_round_trip() {
        let destination_hash = [0x22u8; DESTINATION_LENGTH];
        let mut req = RnsRequest {
            request_id: [0u8; DESTINATION_LENGTH],
            requested_at: 1_700_000_000.0,
            path_hash: [0x11u8; DESTINATION_LENGTH],
            data: Value::Binary(vec![0x01, 0x02, 0x03]),
        };
        let bytes = encode_request_bytes(&req).expect("encode request");
        let decoded = decode_request_bytes_for_destination(destination_hash, &bytes)
            .expect("decode request");
        assert_eq!(decoded.path_hash, req.path_hash);
        assert_eq!(
            decoded.request_id,
            request_id_for_destination(destination_hash, &bytes).expect("request id")
        );

        req.request_id = decoded.request_id;
        let resp = RnsResponse {
            request_id: req.request_id,
            data: Value::Binary(vec![0x04, 0x05]),
        };
        let resp_bytes = encode_response_bytes(&resp).expect("encode response");
        let decoded_resp = decode_response_bytes(&resp_bytes).expect("decode response");
        assert_eq!(decoded_resp.request_id, resp.request_id);
    }
}
