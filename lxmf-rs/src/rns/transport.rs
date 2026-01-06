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
use reticulum::request::{RequestCallbacks, RequestReceiptHandle, RequestTracker};
use reticulum::resource::{
    ResourceAdvertisement, ResourceAdvertisementResult, ResourceCancel, ResourceEvent,
    ResourceHashUpdate, ResourcePart, ResourceProof, ResourceReceiptStatus, ResourceRequest,
    ResourceSendState, ResourceTracker,
    RESOURCE_DEFAULT_MAX_RETRIES, RESOURCE_PART_HEADER_LEN, RESOURCE_PROOF_STATUS_FAILED,
    RESOURCE_PROOF_STATUS_OK,
};
#[cfg(test)]
use reticulum::resource::ResourceSender;
use reticulum::transport::{Transport, TransportConfig};
use rmpv::Value;
use sha2::Digest;
use std::io::Cursor;
use std::time::{Duration, Instant};
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

const RESOURCE_TIMEOUT: Duration = Duration::from_secs(30);
const RESOURCE_PROOF_TIMEOUT: Duration = Duration::from_secs(10);

struct ResourceControlAction {
    link_id: reticulum::hash::AddressHash,
    context: PacketContext,
    payload: Vec<u8>,
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
    request_tracker: RequestTracker,
    resource_tracker: ResourceTracker,
    resource_outgoing: std::collections::HashMap<[u8; DESTINATION_LENGTH], ResourceSendState>,
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
            request_tracker: RequestTracker::new(),
            resource_tracker: ResourceTracker::new(RESOURCE_TIMEOUT),
            resource_outgoing: std::collections::HashMap::new(),
        })
    }

    pub async fn send_message(
        &mut self,
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
            DeliveryOutput::Resource {
                destination,
                data,
                context: _,
            } => {
                self.send_resource_frames(destination, &data, None, false, false)
                    .await?;
            }
            DeliveryOutput::PaperUri(_) => {}
        }

        Ok(())
    }

    pub async fn send_resource_payload(
        &mut self,
        destination: reticulum::hash::AddressHash,
        payload: &[u8],
    ) -> Result<(), RnsError> {
        self.send_resource_frames(destination, payload, None, false, false)
            .await
    }

    async fn send_resource_frames(
        &mut self,
        destination: reticulum::hash::AddressHash,
        payload: &[u8],
        request_id: Option<[u8; DESTINATION_LENGTH]>,
        is_response: bool,
        is_request: bool,
    ) -> Result<(), RnsError> {
        let part_size = PACKET_MDU.saturating_sub(RESOURCE_PART_HEADER_LEN);
        if part_size == 0 {
            return Err(RnsError::Msgpack("invalid resource payload".to_string()));
        }
        let mut state = ResourceSendState::new(
            destination,
            payload,
            part_size,
            request_id,
            is_response,
            is_request,
            RESOURCE_PROOF_TIMEOUT,
            RESOURCE_DEFAULT_MAX_RETRIES,
        )
        .map_err(|_| RnsError::Msgpack("invalid resource payload".to_string()))?;
        let advert_bytes = state.advert.encode();
        self.transport
            .send_to_out_links_with_context(
                &destination,
                &advert_bytes,
                PacketContext::ResourceAdvrtisement,
            )
            .await;
        for part in &state.parts {
            self.transport
                .send_to_out_links_with_context(
                    &destination,
                    &part.encode(),
                    PacketContext::Resource,
                )
                .await;
        }
        state.mark_sent(Instant::now());
        self.resource_outgoing.insert(state.resource_id, state);
        Ok(())
    }

    pub async fn announce(&self) {
        self.transport.send_announce(&self.destination, None).await;
    }

    pub async fn poll_inbound(&mut self) -> Vec<Received> {
        let mut out = Vec::new();
        let mut actions = Vec::new();
        let now = Instant::now();
        while let Ok(event) = self.in_events.try_recv() {
            Self::handle_link_event_inner(
                &mut out,
                &mut actions,
                &event,
                &mut self.request_tracker,
                &mut self.resource_tracker,
                &mut self.resource_outgoing,
                now,
            );
        }
        while let Ok(event) = self.out_events.try_recv() {
            Self::handle_link_event_inner(
                &mut out,
                &mut actions,
                &event,
                &mut self.request_tracker,
                &mut self.resource_tracker,
                &mut self.resource_outgoing,
                now,
            );
        }
        self.request_tracker.poll_timeouts(now);
        for timeout in self.resource_tracker.poll_timeouts(now) {
            if let Some(request_id) = timeout.request_id {
                if timeout.failed {
                    self.request_tracker.mark_failed(request_id);
                }
            }
            if timeout.failed {
                actions.push(ResourceControlAction {
                    link_id: reticulum::hash::AddressHash::new(timeout.link_id),
                    context: PacketContext::ResourceProof,
                    payload: ResourceProof {
                        resource_id: timeout.resource_id,
                        status: RESOURCE_PROOF_STATUS_FAILED,
                    }
                    .encode(),
                });
            }
            if !timeout.failed
                && (!timeout.requested_hashes.is_empty() || timeout.hashmap_exhausted)
            {
                actions.push(ResourceControlAction {
                    link_id: reticulum::hash::AddressHash::new(timeout.link_id),
                    context: PacketContext::ResourceRequest,
                    payload: ResourceRequest {
                        resource_id: timeout.resource_id,
                        hashmap_exhausted: timeout.hashmap_exhausted,
                        last_map_hash: timeout.last_map_hash,
                        requested_hashes: timeout.requested_hashes,
                    }
                    .encode(),
                });
            }
        }
        self.retry_outgoing_resources(now).await;
        for action in actions {
            let _ = self
                .transport
                .send_to_link_id_with_context(&action.link_id, &action.payload, action.context)
                .await;
        }
        out
    }

    fn handle_link_event_inner(
        out: &mut Vec<Received>,
        actions: &mut Vec<ResourceControlAction>,
        event: &LinkEventData,
        request_tracker: &mut RequestTracker,
        resource_tracker: &mut ResourceTracker,
        resource_outgoing: &mut std::collections::HashMap<[u8; DESTINATION_LENGTH], ResourceSendState>,
        now: Instant,
    ) {
        let source_hash = source_hash_from_event(event);
        let count_client_receive = source_hash.is_none();
        let (payload, context) = match &event.event {
            LinkEvent::Data { payload, context } => (payload, context),
            _ => return,
        };

        let payload_slice = payload.as_slice();
        match context {
            PacketContext::None => {
                if let Ok(msg) = LXMessage::decode(payload_slice) {
                    out.push(Received::Message(msg));
                    return;
                }
                if let Ok(received) =
                    decode_propagated_payload(payload_slice, source_hash, count_client_receive)
                {
                    out.push(received);
                    return;
                }
                if decode_propagation_resource_payload(payload_slice).is_ok() {
                    out.push(Received::PropagationResource {
                        payload: payload_slice.to_vec(),
                        source_hash,
                    });
                    return;
                }
                if let Ok(request) = address_hash_to_array(&event.address_hash).and_then(
                    |destination_hash| decode_request_bytes_for_destination(destination_hash, payload_slice),
                ) {
                    if let Some(path) = request_path_for_hash(request.path_hash) {
                        if matches!(path, CONTROL_STATS_PATH | CONTROL_SYNC_PATH | CONTROL_UNPEER_PATH) {
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
                    return;
                }
                if let Ok(response) = decode_response_bytes(payload_slice) {
                    let request_id = response.request_id;
                    out.push(Received::Response {
                        response,
                        source_hash,
                    });
                    request_tracker.record_response(request_id, payload_slice.to_vec());
                }
            }
            PacketContext::Request => {
                if let Ok(request) = address_hash_to_array(&event.address_hash).and_then(
                    |destination_hash| decode_request_bytes_for_destination(destination_hash, payload_slice),
                ) {
                    if let Some(path) = request_path_for_hash(request.path_hash) {
                        if matches!(path, CONTROL_STATS_PATH | CONTROL_SYNC_PATH | CONTROL_UNPEER_PATH) {
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
                }
            }
            PacketContext::Response => {
                if let Ok(response) = decode_response_bytes(payload_slice) {
                    let request_id = response.request_id;
                    out.push(Received::Response {
                        response,
                        source_hash,
                    });
                    request_tracker.record_response(request_id, payload_slice.to_vec());
                }
            }
            PacketContext::Resource
            | PacketContext::ResourceAdvrtisement
            | PacketContext::ResourceRequest
            | PacketContext::ResourceHashUpdate
            | PacketContext::ResourceProof
            | PacketContext::ResourceInitiatorCancel
            | PacketContext::ResourceReceiverCancel => {
                if *context == PacketContext::ResourceAdvrtisement {
                    if let Ok(advert) = ResourceAdvertisement::decode(payload_slice) {
                        let link_id = match address_hash_to_array(&event.id) {
                            Ok(id) => id,
                            Err(_) => return,
                        };
                        if let Ok(result) =
                            resource_tracker.handle_advertisement(advert, link_id, now)
                        {
                            if result == ResourceAdvertisementResult::AlreadyCompleted {
                                actions.push(ResourceControlAction {
                                    link_id: event.id,
                                    context: PacketContext::ResourceProof,
                                    payload: ResourceProof {
                                        resource_id: advert.resource_id,
                                        status: RESOURCE_PROOF_STATUS_OK,
                                    }
                                    .encode(),
                                });
                                return;
                            }
                            if let Some(request_id) = advert.request_id {
                                if advert.is_response {
                                    request_tracker.mark_receiving(request_id, 0.0);
                                }
                            }
                        }
                    }
                    return;
                }
                if *context == PacketContext::ResourceHashUpdate {
                    if let Ok(update) = ResourceHashUpdate::decode(payload_slice) {
                        let _ = resource_tracker.handle_hash_update(update, now);
                    }
                    return;
                }
                if *context == PacketContext::ResourceInitiatorCancel
                    || *context == PacketContext::ResourceReceiverCancel
                {
                    if let Ok(cancel) = ResourceCancel::decode(payload_slice) {
                        if let Some(cancelled) =
                            resource_tracker.handle_cancel(cancel.resource_id)
                        {
                            if cancelled.is_response {
                                if let Some(request_id) = cancelled.request_id {
                                    request_tracker.mark_failed(request_id);
                                }
                            }
                        }
                        if let Some(state) = resource_outgoing.get_mut(&cancel.resource_id) {
                            state.mark_failed(now);
                            if state.is_request {
                                if let Some(request_id) = state.request_id {
                                    request_tracker.mark_failed(request_id);
                                }
                            }
                        }
                    }
                    return;
                }
                if *context == PacketContext::ResourceProof {
                    if let Ok(proof) = ResourceProof::decode(payload_slice) {
                        if let Some(state) = resource_outgoing.get_mut(&proof.resource_id) {
                            if state.handle_proof(&proof) {
                                if state.is_request {
                                    if let Some(request_id) = state.request_id {
                                        if proof.status == RESOURCE_PROOF_STATUS_OK {
                                            request_tracker.mark_delivered(request_id);
                                        } else {
                                            request_tracker.mark_failed(request_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                if *context == PacketContext::ResourceRequest {
                    if let Ok(request) = ResourceRequest::decode(payload_slice) {
                        if let Some(state) = resource_outgoing.get_mut(&request.resource_id) {
                            let parts = state.handle_request(&request);
                            for part in &parts {
                                actions.push(ResourceControlAction {
                                    link_id: event.id,
                                    context: PacketContext::Resource,
                                    payload: part.encode(),
                                });
                            }
                            if request.hashmap_exhausted {
                                if let Some(last_hash) = request.last_map_hash {
                                    let mut hashmap = Vec::new();
                                    let mut start_index = 0usize;
                                    for (idx, part) in state.parts.iter().enumerate() {
                                        let part_hash = reticulum::hash::Hash::new_from_slice(&part.data);
                                        let mut map_hash = [0u8; reticulum::resource::RESOURCE_MAPHASH_LEN];
                                        map_hash.copy_from_slice(
                                            &part_hash.as_slice()[..reticulum::resource::RESOURCE_MAPHASH_LEN],
                                        );
                                        if map_hash == last_hash {
                                            start_index = idx + 1;
                                            break;
                                        }
                                    }
                                    for part in state.parts.iter().skip(start_index) {
                                        let part_hash = reticulum::hash::Hash::new_from_slice(&part.data);
                                        hashmap.extend_from_slice(
                                            &part_hash.as_slice()[..reticulum::resource::RESOURCE_MAPHASH_LEN],
                                        );
                                    }
                                    actions.push(ResourceControlAction {
                                        link_id: event.id,
                                        context: PacketContext::ResourceHashUpdate,
                                        payload: ResourceHashUpdate {
                                            resource_id: request.resource_id,
                                            segment: 0,
                                            hashmap,
                                        }
                                        .encode(),
                                    });
                                }
                            }
                            if !parts.is_empty() {
                                state.mark_sent(now);
                            }
                        }
                    }
                    return;
                }
                if *context == PacketContext::Resource {
                    if let Ok(part) = ResourcePart::decode(payload_slice) {
                        if let Ok(Some(resource_event)) = resource_tracker.handle_part(part) {
                            match resource_event {
                                ResourceEvent::Progress(progress) => {
                                    if progress.is_response {
                                        if let Some(request_id) = progress.request_id {
                                            request_tracker.mark_receiving(
                                                request_id,
                                                progress.progress,
                                            );
                                        }
                                    }
                                }
                                ResourceEvent::Complete(completion) => {
                                    actions.push(ResourceControlAction {
                                        link_id: event.id,
                                        context: PacketContext::ResourceProof,
                                        payload: ResourceProof {
                                            resource_id: completion.resource_id,
                                            status: RESOURCE_PROOF_STATUS_OK,
                                        }
                                        .encode(),
                                    });
                                    if let Some(request_id) = completion.request_id {
                                        if completion.is_response {
                                            if let Ok(response) =
                                                decode_response_bytes(&completion.data)
                                            {
                                                let response_id = response.request_id;
                                                out.push(Received::Response {
                                                    response,
                                                    source_hash,
                                                });
                                                request_tracker.record_response(
                                                    response_id,
                                                    completion.data.clone(),
                                                );
                                            }
                                        } else if let Ok(mut request) =
                                            decode_request_bytes(&completion.data)
                                        {
                                            request.request_id = request_id;
                                            if let Some(path) =
                                                request_path_for_hash(request.path_hash)
                                            {
                                                if matches!(
                                                    path,
                                                    CONTROL_STATS_PATH
                                                        | CONTROL_SYNC_PATH
                                                        | CONTROL_UNPEER_PATH
                                                ) {
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
                                        }
                                    } else {
                                        if let Ok(msg) = LXMessage::decode(&completion.data) {
                                            out.push(Received::Message(msg));
                                        } else if let Ok(received) = decode_propagated_payload(
                                            &completion.data,
                                            source_hash,
                                            count_client_receive,
                                        ) {
                                            out.push(received);
                                        } else if decode_propagation_resource_payload(
                                            &completion.data,
                                        )
                                        .is_ok()
                                        {
                                            out.push(Received::PropagationResource {
                                                payload: completion.data,
                                                source_hash,
                                            });
                                        }
                                    }
                                }
                            }
                            return;
                        }
                    }
                    if decode_propagation_resource_payload(payload_slice).is_ok() {
                        out.push(Received::PropagationResource {
                            payload: payload_slice.to_vec(),
                            source_hash,
                        });
                    }
                    return;
                }
                if decode_propagation_resource_payload(payload_slice).is_ok() {
                    out.push(Received::PropagationResource {
                        payload: payload_slice.to_vec(),
                        source_hash,
                    });
                }
            }
            _ => {}
        }
    }

    async fn retry_outgoing_resources(&mut self, now: Instant) {
        let mut remove = Vec::new();
        for (resource_id, state) in self.resource_outgoing.iter_mut() {
            match state.status() {
                ResourceReceiptStatus::Completed => {
                    if state.is_request {
                        if let Some(request_id) = state.request_id {
                            self.request_tracker.mark_delivered(request_id);
                        }
                    }
                    remove.push(*resource_id);
                    continue;
                }
                ResourceReceiptStatus::Failed => {
                    if state.is_request {
                        if let Some(request_id) = state.request_id {
                            self.request_tracker.mark_failed(request_id);
                        }
                    }
                    remove.push(*resource_id);
                    continue;
                }
                ResourceReceiptStatus::Sent => {}
            }

            if let Some(parts) = state.poll_retry(now) {
                let advert_bytes = state.advert.encode();
                self.transport
                    .send_to_out_links_with_context(
                        &state.destination,
                        &advert_bytes,
                        PacketContext::ResourceAdvrtisement,
                    )
                    .await;
                for part in parts {
                    self.transport
                        .send_to_out_links_with_context(
                            &state.destination,
                            &part.encode(),
                            PacketContext::Resource,
                        )
                        .await;
                }
                state.mark_sent(now);
            }
        }
        for resource_id in remove {
            self.resource_outgoing.remove(&resource_id);
        }
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
        &mut self,
        destination: Identity,
        req: &mut RnsRequest,
    ) -> Result<(), RnsError> {
        let payload = encode_request_bytes(req)?;
        if payload.len() <= PACKET_MDU {
            let packet = self.build_request_packet(destination, req)?;
            self.transport.send_packet(packet).await;
        } else {
            let dest = SingleOutputDestination::new(destination, DestinationName::new("lxmf", "req"));
            ensure_request_id(req, &payload);
            self.send_resource_frames(
                dest.desc.address_hash,
                &payload,
                Some(req.request_id),
                false,
                true,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn send_response(
        &mut self,
        destination: Identity,
        resp: &RnsResponse,
    ) -> Result<(), RnsError> {
        let payload = encode_response_bytes(resp)?;
        if payload.len() <= PACKET_MDU {
            let packet = self.build_response_packet(destination, resp)?;
            self.transport.send_packet(packet).await;
        } else {
            let dest = SingleOutputDestination::new(destination, DestinationName::new("lxmf", "req"));
            self.send_resource_frames(
                dest.desc.address_hash,
                &payload,
                Some(resp.request_id),
                true,
                false,
            )
            .await?;
        }
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
        &mut self,
        destination_hash: [u8; DESTINATION_LENGTH],
        req: &mut RnsRequest,
    ) -> Result<(), RnsError> {
        let payload = encode_request_bytes(req)?;
        if payload.len() <= PACKET_MDU {
            let packet = self.build_request_packet_for_hash(destination_hash, req)?;
            self.transport.send_packet(packet).await;
        } else {
            ensure_request_id(req, &payload);
            self.send_resource_frames(
                reticulum::hash::AddressHash::new(destination_hash),
                &payload,
                Some(req.request_id),
                false,
                true,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn send_response_to_hash(
        &mut self,
        destination_hash: [u8; DESTINATION_LENGTH],
        resp: &RnsResponse,
    ) -> Result<(), RnsError> {
        let payload = encode_response_bytes(resp)?;
        if payload.len() <= PACKET_MDU {
            let packet = self.build_response_packet_for_hash(destination_hash, resp)?;
            self.transport.send_packet(packet).await;
        } else {
            self.send_resource_frames(
                reticulum::hash::AddressHash::new(destination_hash),
                &payload,
                Some(resp.request_id),
                true,
                false,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn send_request_with_receipt_to_hash(
        &mut self,
        destination_hash: [u8; DESTINATION_LENGTH],
        req: &mut RnsRequest,
        timeout: Duration,
    ) -> Result<RequestReceiptHandle, RnsError> {
        let payload = encode_request_bytes(req)?;
        if payload.len() <= PACKET_MDU {
            let packet = self.build_request_packet_for_hash(destination_hash, req)?;
            let receipt = self.request_tracker.register_request(
                req.request_id,
                timeout,
                RequestCallbacks::default(),
            );
            self.transport.send_packet(packet).await;
            self.request_tracker.mark_delivered(req.request_id);
            Ok(receipt)
        } else {
            ensure_request_id(req, &payload);
            let receipt = self.request_tracker.register_request(
                req.request_id,
                timeout,
                RequestCallbacks::default(),
            );
            self.send_resource_frames(
                reticulum::hash::AddressHash::new(destination_hash),
                &payload,
                Some(req.request_id),
                false,
                true,
            )
            .await?;
            self.request_tracker.mark_delivered(req.request_id);
            Ok(receipt)
        }
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

fn ensure_request_id(req: &mut RnsRequest, payload: &[u8]) {
    if req.request_id.iter().all(|byte| *byte == 0) {
        req.request_id = request_id_from_payload(payload);
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
    use reticulum::request::RequestStatus;
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
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&payload),
                context: PacketContext::None,
            },
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
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&buf),
                context: PacketContext::None,
            },
        );

        let source_hash = source_hash_from_event(&event);
        assert!(source_hash.is_none());
        let received = decode_propagated_payload(&buf, source_hash, true);
        assert!(received.is_err());

        let resource = decode_propagation_resource_payload(&buf).expect("resource");
        assert_eq!(resource.len(), 2);
    }

    #[test]
    fn handle_link_event_request_context_decodes_request() {
        let destination_hash = [0x22u8; DESTINATION_LENGTH];
        let request = RnsRequest {
            request_id: [0u8; DESTINATION_LENGTH],
            requested_at: 1_700_000_000.0,
            path_hash: [0x11u8; DESTINATION_LENGTH],
            data: Value::Binary(vec![0x01, 0x02, 0x03]),
        };
        let payload = encode_request_bytes(&request).expect("encode request");
        let event = LinkEventData::new(
            AddressHash::new_empty(),
            AddressHash::new(destination_hash),
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&payload),
                context: PacketContext::Request,
            },
        );

        let mut out = Vec::new();
        let mut actions = Vec::new();
        let mut request_tracker = RequestTracker::new();
        let mut resource_tracker = ResourceTracker::new(RESOURCE_TIMEOUT);
        let mut resource_outgoing = std::collections::HashMap::new();
        RnsNodeRouter::handle_link_event_inner(
            &mut out,
            &mut actions,
            &event,
            &mut request_tracker,
            &mut resource_tracker,
            &mut resource_outgoing,
            Instant::now(),
        );
        assert_eq!(out.len(), 1);
        match &out[0] {
            Received::Request { request, .. } => assert_eq!(request.path_hash, [0x11u8; 16]),
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn handle_link_event_resource_context_decodes_resource() {
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
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&buf),
                context: PacketContext::Resource,
            },
        );

        let mut out = Vec::new();
        let mut actions = Vec::new();
        let mut request_tracker = RequestTracker::new();
        let mut resource_tracker = ResourceTracker::new(RESOURCE_TIMEOUT);
        let mut resource_outgoing = std::collections::HashMap::new();
        RnsNodeRouter::handle_link_event_inner(
            &mut out,
            &mut actions,
            &event,
            &mut request_tracker,
            &mut resource_tracker,
            &mut resource_outgoing,
            Instant::now(),
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], Received::PropagationResource { .. }));
    }

    #[test]
    fn resource_response_completes_request_receipt() {
        let request_id = [0x33u8; DESTINATION_LENGTH];
        let response = RnsResponse {
            request_id,
            data: Value::Binary(vec![0x01, 0x02, 0x03]),
        };
        let payload = encode_response_bytes(&response).expect("encode response");
        let (advert, parts) = ResourceSender::split(
            &payload,
            16,
            Some(request_id),
            true,
            false,
        )
        .expect("split");

        let mut out = Vec::new();
        let mut actions = Vec::new();
        let mut request_tracker = RequestTracker::new();
        let receipt = request_tracker.register_request(
            request_id,
            Duration::from_secs(10),
            RequestCallbacks::default(),
        );
        let mut resource_tracker = ResourceTracker::new(RESOURCE_TIMEOUT);
        let mut resource_outgoing = std::collections::HashMap::new();

        let adv_event = LinkEventData::new(
            AddressHash::new_empty(),
            AddressHash::new_empty(),
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&advert.encode()),
                context: PacketContext::ResourceAdvrtisement,
            },
        );
        RnsNodeRouter::handle_link_event_inner(
            &mut out,
            &mut actions,
            &adv_event,
            &mut request_tracker,
            &mut resource_tracker,
            &mut resource_outgoing,
            Instant::now(),
        );

        for part in parts {
            let part_event = LinkEventData::new(
                AddressHash::new_empty(),
                AddressHash::new_empty(),
                LinkEvent::Data {
                    payload: LinkPayload::new_from_slice(&part.encode()),
                    context: PacketContext::Resource,
                },
            );
            RnsNodeRouter::handle_link_event_inner(
                &mut out,
                &mut actions,
                &part_event,
                &mut request_tracker,
                &mut resource_tracker,
                &mut resource_outgoing,
                Instant::now(),
            );
        }

        assert!(out.iter().any(|received| matches!(received, Received::Response { .. })));
        let receipt_guard = receipt.lock().unwrap();
        assert_eq!(receipt_guard.status, RequestStatus::Ready);
        assert!(receipt_guard.response.is_some());
    }

    #[test]
    fn resource_loopback_adv_req_parts_proof() {
        let payload = vec![0x5au8; 64];
        let destination = AddressHash::new([0x11u8; DESTINATION_LENGTH]);
        let sender_state = ResourceSendState::new(
            destination,
            &payload,
            16,
            None,
            false,
            false,
            Duration::from_secs(5),
            1,
        )
        .expect("state");

        let resource_id = sender_state.resource_id;
        let parts = sender_state.parts.clone();
        let advert = sender_state.advert.encode();

        let mut sender_outgoing = std::collections::HashMap::new();
        sender_outgoing.insert(resource_id, sender_state);
        let mut sender_out = Vec::new();
        let mut sender_actions = Vec::new();
        let mut sender_requests = RequestTracker::new();
        let mut sender_tracker = ResourceTracker::new(RESOURCE_TIMEOUT);

        let mut receiver_out = Vec::new();
        let mut receiver_actions = Vec::new();
        let mut receiver_requests = RequestTracker::new();
        let mut receiver_tracker = ResourceTracker::new(RESOURCE_TIMEOUT);
        let mut receiver_outgoing = std::collections::HashMap::new();

        let link_id = AddressHash::new([0x22u8; DESTINATION_LENGTH]);
        let event_hash = AddressHash::new_empty();
        let now = Instant::now();

        let adv_event = LinkEventData::new(
            link_id,
            event_hash,
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&advert),
                context: PacketContext::ResourceAdvrtisement,
            },
        );
        RnsNodeRouter::handle_link_event_inner(
            &mut receiver_out,
            &mut receiver_actions,
            &adv_event,
            &mut receiver_requests,
            &mut receiver_tracker,
            &mut receiver_outgoing,
            now,
        );

        for (idx, part) in parts.iter().enumerate() {
            if idx == 1 {
                continue;
            }
            let part_event = LinkEventData::new(
                link_id,
                event_hash,
                LinkEvent::Data {
                    payload: LinkPayload::new_from_slice(&part.encode()),
                    context: PacketContext::Resource,
                },
            );
            RnsNodeRouter::handle_link_event_inner(
                &mut receiver_out,
                &mut receiver_actions,
                &part_event,
                &mut receiver_requests,
                &mut receiver_tracker,
                &mut receiver_outgoing,
                now,
            );
        }

        let mut missing_hash = [0u8; reticulum::resource::RESOURCE_MAPHASH_LEN];
        let part_hash = reticulum::hash::Hash::new_from_slice(&parts[1].data);
        missing_hash.copy_from_slice(
            &part_hash.as_slice()[..reticulum::resource::RESOURCE_MAPHASH_LEN],
        );
        let request = ResourceRequest {
            resource_id,
            hashmap_exhausted: false,
            last_map_hash: None,
            requested_hashes: vec![missing_hash],
        };
        let req_event = LinkEventData::new(
            link_id,
            event_hash,
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&request.encode()),
                context: PacketContext::ResourceRequest,
            },
        );
        RnsNodeRouter::handle_link_event_inner(
            &mut sender_out,
            &mut sender_actions,
            &req_event,
            &mut sender_requests,
            &mut sender_tracker,
            &mut sender_outgoing,
            now,
        );

        assert!(!sender_actions.is_empty());
        for action in sender_actions.drain(..) {
            if action.context != PacketContext::Resource {
                continue;
            }
            let part_event = LinkEventData::new(
                link_id,
                event_hash,
                LinkEvent::Data {
                    payload: LinkPayload::new_from_slice(&action.payload),
                    context: PacketContext::Resource,
                },
            );
            RnsNodeRouter::handle_link_event_inner(
                &mut receiver_out,
                &mut receiver_actions,
                &part_event,
                &mut receiver_requests,
                &mut receiver_tracker,
                &mut receiver_outgoing,
                now,
            );
        }

        assert!(receiver_actions
            .iter()
            .any(|action| action.context == PacketContext::ResourceProof));

        for action in receiver_actions.drain(..) {
            if action.context != PacketContext::ResourceProof {
                continue;
            }
            let proof_event = LinkEventData::new(
                link_id,
                event_hash,
                LinkEvent::Data {
                    payload: LinkPayload::new_from_slice(&action.payload),
                    context: PacketContext::ResourceProof,
                },
            );
            RnsNodeRouter::handle_link_event_inner(
                &mut sender_out,
                &mut sender_actions,
                &proof_event,
                &mut sender_requests,
                &mut sender_tracker,
                &mut sender_outgoing,
                now,
            );
        }

        let status = sender_outgoing
            .get(&resource_id)
            .expect("state")
            .status();
        assert_eq!(status, ResourceReceiptStatus::Completed);
    }

    #[test]
    fn resource_advertisement_duplicate_sends_proof() {
        let payload = vec![0x55u8; 48];
        let (advert, parts) =
            ResourceSender::split(&payload, 16, None, false, false).expect("split");
        let link_id = AddressHash::new([0x11u8; DESTINATION_LENGTH]);

        let mut out = Vec::new();
        let mut actions = Vec::new();
        let mut request_tracker = RequestTracker::new();
        let mut resource_tracker = ResourceTracker::new(RESOURCE_TIMEOUT);
        let mut resource_outgoing = std::collections::HashMap::new();

        let adv_event = LinkEventData::new(
            link_id,
            AddressHash::new_empty(),
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&advert.encode()),
                context: PacketContext::ResourceAdvrtisement,
            },
        );
        RnsNodeRouter::handle_link_event_inner(
            &mut out,
            &mut actions,
            &adv_event,
            &mut request_tracker,
            &mut resource_tracker,
            &mut resource_outgoing,
            Instant::now(),
        );

        for part in parts {
            let part_event = LinkEventData::new(
                link_id,
                AddressHash::new_empty(),
                LinkEvent::Data {
                    payload: LinkPayload::new_from_slice(&part.encode()),
                    context: PacketContext::Resource,
                },
            );
            RnsNodeRouter::handle_link_event_inner(
                &mut out,
                &mut actions,
                &part_event,
                &mut request_tracker,
                &mut resource_tracker,
                &mut resource_outgoing,
                Instant::now(),
            );
        }

        actions.clear();
        let adv_event = LinkEventData::new(
            link_id,
            AddressHash::new_empty(),
            LinkEvent::Data {
                payload: LinkPayload::new_from_slice(&advert.encode()),
                context: PacketContext::ResourceAdvrtisement,
            },
        );
        RnsNodeRouter::handle_link_event_inner(
            &mut out,
            &mut actions,
            &adv_event,
            &mut request_tracker,
            &mut resource_tracker,
            &mut resource_outgoing,
            Instant::now(),
        );

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].context, PacketContext::ResourceProof);
        let proof = ResourceProof::decode(&actions[0].payload).expect("proof");
        assert_eq!(proof.resource_id, advert.resource_id);
        assert_eq!(proof.status, RESOURCE_PROOF_STATUS_OK);
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
