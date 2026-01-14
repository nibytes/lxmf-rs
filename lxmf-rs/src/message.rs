use crate::constants::{
    COST_TICKET, DESTINATION_LENGTH, HASH_LENGTH, LXMF_OVERHEAD, REPR_UNKNOWN, SIGNATURE_LENGTH,
    STATE_GENERATING, STAMP_SIZE, TICKET_LENGTH, WORKBLOCK_EXPAND_ROUNDS,
    WORKBLOCK_EXPAND_ROUNDS_PEERING, WORKBLOCK_EXPAND_ROUNDS_PN,
};
use crate::error::LXMFError;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use ed25519_dalek::{Signer, Verifier};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use rmpv::Value;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub struct LXMessage {
    pub destination_hash: [u8; DESTINATION_LENGTH],
    pub source_hash: [u8; DESTINATION_LENGTH],
    pub timestamp: f64,
    pub title: Vec<u8>,
    pub content: Vec<u8>,
    pub fields: Value,
    pub stamp: Option<Vec<u8>>,
    pub stamp_cost: Option<u32>,
    pub stamp_value: Option<u32>,
    pub stamp_valid: bool,
    pub stamp_checked: bool,
    pub outbound_ticket: Option<Vec<u8>>,
    pub include_ticket: bool,
    pub defer_stamp: bool,
    pub state: u8,
    pub method: Option<u8>,
    pub representation: u8,
    pub transport_encrypted: Option<bool>,
    pub transport_encryption: Option<String>,
    pub signature: Option<[u8; SIGNATURE_LENGTH]>,
    pub message_id: Option<[u8; 32]>,
}

impl LXMessage {
    pub fn new(destination_hash: [u8; DESTINATION_LENGTH], source_hash: [u8; DESTINATION_LENGTH]) -> Self {
        Self {
            destination_hash,
            source_hash,
            timestamp: 0.0,
            title: Vec::new(),
            content: Vec::new(),
            fields: Value::Map(Vec::new()),
            stamp: None,
            stamp_cost: None,
            stamp_value: None,
            stamp_valid: false,
            stamp_checked: false,
            outbound_ticket: None,
            include_ticket: false,
            defer_stamp: true,
            state: STATE_GENERATING,
            method: None,
            representation: REPR_UNKNOWN,
            transport_encrypted: None,
            transport_encryption: None,
            signature: None,
            message_id: None,
        }
    }

    pub fn with_strings(
        destination_hash: [u8; DESTINATION_LENGTH],
        source_hash: [u8; DESTINATION_LENGTH],
        timestamp: f64,
        title: &str,
        content: &str,
        fields: Value,
    ) -> Self {
        Self {
            destination_hash,
            source_hash,
            timestamp,
            title: title.as_bytes().to_vec(),
            content: content.as_bytes().to_vec(),
            fields,
            stamp: None,
            stamp_cost: None,
            stamp_value: None,
            stamp_valid: false,
            stamp_checked: false,
            outbound_ticket: None,
            include_ticket: false,
            defer_stamp: true,
            state: STATE_GENERATING,
            method: None,
            representation: REPR_UNKNOWN,
            transport_encrypted: None,
            transport_encryption: None,
            signature: None,
            message_id: None,
        }
    }

    pub fn set_outbound_ticket(&mut self, ticket: Vec<u8>) {
        self.outbound_ticket = Some(ticket);
    }

    pub fn set_stamp_cost(&mut self, cost: u32) {
        self.stamp_cost = Some(cost);
    }

    pub fn set_defer_stamp(&mut self, defer: bool) {
        self.defer_stamp = defer;
    }

    pub fn encode_signed(&mut self, signing_key: &SigningKey) -> Result<Vec<u8>, LXMFError> {
        let payload_without_stamp = self.pack_payload(false)?;
        let message_id = compute_message_id(&self.destination_hash, &self.source_hash, &payload_without_stamp);
        let signed_part = build_signed_part(&self.destination_hash, &self.source_hash, &payload_without_stamp, &message_id);
        self.message_id = Some(message_id);

        if !self.defer_stamp {
            self.ensure_stamp()?;
        }

        let signature = signing_key.sign(&signed_part).to_bytes();

        self.signature = Some(signature);

        self.encode_with_signature()
    }

    pub fn encode_with_signature(&mut self) -> Result<Vec<u8>, LXMFError> {
        let signature = self.signature.ok_or(LXMFError::MissingSignature)?;
        let payload_with_stamp = self.pack_payload(self.stamp.is_some())?;

        let mut out = Vec::with_capacity(
            DESTINATION_LENGTH * 2 + SIGNATURE_LENGTH + payload_with_stamp.len(),
        );
        out.extend_from_slice(&self.destination_hash);
        out.extend_from_slice(&self.source_hash);
        out.extend_from_slice(&signature);
        out.extend_from_slice(&payload_with_stamp);

        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LXMFError> {
        let min_len = DESTINATION_LENGTH * 2 + SIGNATURE_LENGTH + 1;
        if bytes.len() < min_len {
            return Err(LXMFError::InvalidLength);
        }

        let destination_hash = bytes[..DESTINATION_LENGTH]
            .try_into()
            .map_err(|_| LXMFError::InvalidLength)?;
        let source_hash = bytes[DESTINATION_LENGTH..DESTINATION_LENGTH * 2]
            .try_into()
            .map_err(|_| LXMFError::InvalidLength)?;
        let signature: [u8; SIGNATURE_LENGTH] = bytes
            [DESTINATION_LENGTH * 2..DESTINATION_LENGTH * 2 + SIGNATURE_LENGTH]
            .try_into()
            .map_err(|_| LXMFError::InvalidLength)?;
        let packed_payload = &bytes[DESTINATION_LENGTH * 2 + SIGNATURE_LENGTH..];

        let payload_value = rmpv::decode::read_value(&mut Cursor::new(packed_payload))?;
        let payload_array = match payload_value {
            Value::Array(arr) => arr,
            _ => return Err(LXMFError::InvalidPayload),
        };

        if payload_array.len() < 4 {
            return Err(LXMFError::InvalidPayload);
        }

        let timestamp = value_to_f64(&payload_array[0])?;
        let title = value_to_bytes(&payload_array[1])?;
        let content = value_to_bytes(&payload_array[2])?;
        let fields = payload_array[3].clone();

        let stamp = if payload_array.len() > 4 {
            Some(value_to_bytes(&payload_array[4])?)
        } else {
            None
        };

        let payload_without_stamp = pack_payload_values(&payload_array[..4])?;
        let message_id = compute_message_id(&destination_hash, &source_hash, &payload_without_stamp);

        Ok(Self {
            destination_hash,
            source_hash,
            timestamp,
            title,
            content,
            fields,
            stamp,
            stamp_cost: None,
            stamp_value: None,
            stamp_valid: false,
            stamp_checked: false,
            outbound_ticket: None,
            include_ticket: false,
            defer_stamp: true,
            state: STATE_GENERATING,
            method: None,
            representation: REPR_UNKNOWN,
            transport_encrypted: None,
            transport_encryption: None,
            signature: Some(signature),
            message_id: Some(message_id),
        })
    }

    pub fn verify(&self, verifying_key: &VerifyingKey) -> Result<bool, LXMFError> {
        let signature = self.signature.ok_or(LXMFError::MissingSignature)?;
        let payload_without_stamp = self.pack_payload(false)?;
        let message_id = compute_message_id(&self.destination_hash, &self.source_hash, &payload_without_stamp);
        let signed_part = build_signed_part(&self.destination_hash, &self.source_hash, &payload_without_stamp, &message_id);
        let signature = Signature::from_bytes(&signature);

        Ok(verifying_key.verify(&signed_part, &signature).is_ok())
    }

    pub fn validate_stamp(
        &mut self,
        stamp_cost: Option<u32>,
        ticket: Option<&[u8]>,
    ) -> Result<bool, LXMFError> {
        self.stamp_checked = true;

        let stamp = match &self.stamp {
            Some(stamp) => stamp.as_slice(),
            None => {
                self.stamp_valid = false;
                return Ok(false);
            }
        };

        let payload_without_stamp = self.pack_payload(false)?;
        let message_id = compute_message_id(&self.destination_hash, &self.source_hash, &payload_without_stamp);
        self.message_id = Some(message_id);

        if let Some(ticket) = ticket {
            if ticket.len() == TICKET_LENGTH && stamp.len() == TICKET_LENGTH {
                let mut material = Vec::with_capacity(TICKET_LENGTH + HASH_LENGTH);
                material.extend_from_slice(ticket);
                material.extend_from_slice(&message_id);
                let expected = truncated_hash(&material);
                if expected.as_slice() == stamp {
                    self.stamp_value = Some(COST_TICKET);
                    self.stamp_valid = true;
                    return Ok(true);
                }
            }
        }

        if let Some(cost) = stamp_cost {
            let workblock = stamp_workblock(&message_id, WORKBLOCK_EXPAND_ROUNDS)?;
            if stamp_valid(&workblock, stamp, cost) {
                let value = stamp_value(&workblock, stamp);
                self.stamp_value = Some(value);
                self.stamp_valid = true;
                return Ok(true);
            }
        }

        self.stamp_valid = false;
        Ok(false)
    }

    fn pack_payload(&self, include_stamp: bool) -> Result<Vec<u8>, LXMFError> {
        let mut values = vec![
            Value::F64(self.timestamp),
            Value::Binary(self.title.clone()),
            Value::Binary(self.content.clone()),
            self.fields.clone(),
        ];

        if include_stamp {
            if let Some(stamp) = &self.stamp {
                values.push(Value::Binary(stamp.clone()));
            }
        }

        pack_payload_values(&values)
    }

    fn ensure_stamp(&mut self) -> Result<(), LXMFError> {
        if self.stamp.is_some() {
            return Ok(());
        }

        let message_id = self.message_id.ok_or(LXMFError::MissingSignature)?;

        if let Some(ticket) = &self.outbound_ticket {
            if ticket.len() == TICKET_LENGTH {
                let mut material = Vec::with_capacity(TICKET_LENGTH + HASH_LENGTH);
                material.extend_from_slice(ticket);
                material.extend_from_slice(&message_id);
                let stamp = truncated_hash(&material);
                self.stamp = Some(stamp.to_vec());
                self.stamp_value = Some(COST_TICKET);
                self.stamp_valid = true;
                self.stamp_checked = true;
                return Ok(());
            }
        }

        if let Some(cost) = self.stamp_cost {
            let (stamp, value) = generate_stamp(&message_id, cost, WORKBLOCK_EXPAND_ROUNDS);
            if let Some(stamp) = stamp {
                self.stamp = Some(stamp.to_vec());
                self.stamp_value = Some(value);
                self.stamp_valid = true;
                self.stamp_checked = true;
            }
        }

        Ok(())
    }

    pub fn paper_packed_from_encrypted(&self, encrypted_payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(DESTINATION_LENGTH + encrypted_payload.len());
        out.extend_from_slice(&self.destination_hash);
        out.extend_from_slice(encrypted_payload);
        out
    }

    pub fn propagation_packed_from_encrypted(
        &self,
        encrypted_payload: &[u8],
        timestamp: f64,
        propagation_stamp: Option<&[u8]>,
    ) -> Result<Vec<u8>, LXMFError> {
        let mut lxm_data = Vec::with_capacity(
            DESTINATION_LENGTH + encrypted_payload.len() + propagation_stamp.map(|s| s.len()).unwrap_or(0),
        );
        lxm_data.extend_from_slice(&self.destination_hash);
        lxm_data.extend_from_slice(encrypted_payload);
        if let Some(stamp) = propagation_stamp {
            lxm_data.extend_from_slice(stamp);
        }

        let container = Value::Array(vec![
            Value::F64(timestamp),
            Value::Array(vec![Value::Binary(lxm_data)]),
        ]);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &container)?;
        Ok(buf)
    }

    pub fn transient_id_from_encrypted(&self, encrypted_payload: &[u8]) -> [u8; HASH_LENGTH] {
        let mut data = Vec::with_capacity(DESTINATION_LENGTH + encrypted_payload.len());
        data.extend_from_slice(&self.destination_hash);
        data.extend_from_slice(encrypted_payload);
        full_hash(&data)
    }

    pub fn generate_propagation_stamp(
        &self,
        encrypted_payload: &[u8],
        target_cost: u32,
    ) -> Result<([u8; HASH_LENGTH], [u8; STAMP_SIZE], u32), LXMFError> {
        let transient_id = self.transient_id_from_encrypted(encrypted_payload);
        let (stamp, value) = generate_stamp(&transient_id, target_cost, WORKBLOCK_EXPAND_ROUNDS_PN);
        let stamp = stamp.ok_or(LXMFError::InvalidPayload)?;
        Ok((transient_id, stamp, value))
    }

    pub fn as_uri_from_paper(&self, paper_packed: &[u8]) -> String {
        let encoded = URL_SAFE_NO_PAD.encode(paper_packed);
        format!("lxm://{encoded}")
    }

    pub fn as_qr_from_paper(&self, paper_packed: &[u8]) -> Result<String, LXMFError> {
        let uri = self.as_uri_from_paper(paper_packed);
        let code = qrcode::QrCode::new(uri.as_bytes())?;
        Ok(code.render::<char>().quiet_zone(false).build())
    }

    pub fn packed_container(&mut self) -> Result<Vec<u8>, LXMFError> {
        let lxmf_bytes = self.encode_with_signature()?;
        let mut entries = Vec::new();
        entries.push((Value::String("lxmf_bytes".into()), Value::Binary(lxmf_bytes)));

        entries.push((Value::String("state".into()), Value::Integer(self.state.into())));
        if let Some(method) = self.method {
            entries.push((Value::String("method".into()), Value::Integer(method.into())));
        }
        if let Some(flag) = self.transport_encrypted {
            entries.push((Value::String("transport_encrypted".into()), Value::Boolean(flag)));
        }
        if let Some(enc) = &self.transport_encryption {
            entries.push((Value::String("transport_encryption".into()), Value::String(enc.clone().into())));
        }

        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &Value::Map(entries))?;
        Ok(buf)
    }

    pub fn unpack_from_container(bytes: &[u8]) -> Result<Self, LXMFError> {
        let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
        let entries = match value {
            Value::Map(map) => map,
            _ => return Err(LXMFError::InvalidPayload),
        };

        let mut lxmf_bytes = None;
        let mut state = None;
        let mut method = None;
        let mut transport_encrypted = None;
        let mut transport_encryption = None;

        for (key, val) in entries {
            let key_str = match key {
                Value::String(s) => s.as_str().map(|v| v.to_string()),
                _ => None,
            };
            let Some(key_str) = key_str else { continue };

            match key_str.as_str() {
                "lxmf_bytes" => {
                    if let Value::Binary(b) = val {
                        lxmf_bytes = Some(b);
                    }
                }
                "state" => {
                    if let Value::Integer(i) = val {
                        state = i.as_u64().and_then(|v| u8::try_from(v).ok());
                    }
                }
                "method" => {
                    if let Value::Integer(i) = val {
                        method = i.as_u64().and_then(|v| u8::try_from(v).ok());
                    }
                }
                "transport_encrypted" => {
                    if let Value::Boolean(b) = val {
                        transport_encrypted = Some(b);
                    }
                }
                "transport_encryption" => {
                    if let Value::String(s) = val {
                        transport_encryption = s.as_str().map(|v| v.to_string());
                    }
                }
                _ => {}
            }
        }

        let lxmf_bytes = lxmf_bytes.ok_or(LXMFError::InvalidPayload)?;
        let mut msg = LXMessage::decode(&lxmf_bytes)?;
        msg.state = state.unwrap_or(STATE_GENERATING);
        msg.method = method;
        msg.transport_encrypted = transport_encrypted;
        msg.transport_encryption = transport_encryption;
        Ok(msg)
    }

    pub fn unpack_from_reader<R: Read>(reader: &mut R) -> Result<Self, LXMFError> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Self::unpack_from_container(&buf)
    }

    pub fn unpack_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, LXMFError> {
        let mut file = std::fs::File::open(path)?;
        Self::unpack_from_reader(&mut file)
    }
}

fn pack_payload_values(values: &[Value]) -> Result<Vec<u8>, LXMFError> {
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &Value::Array(values.to_vec()))?;
    Ok(buf)
}

fn compute_message_id(
    destination_hash: &[u8; DESTINATION_LENGTH],
    source_hash: &[u8; DESTINATION_LENGTH],
    packed_payload: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(destination_hash);
    hasher.update(source_hash);
    hasher.update(packed_payload);
    hasher.finalize().into()
}

fn full_hash(data: &[u8]) -> [u8; HASH_LENGTH] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn truncated_hash(data: &[u8]) -> [u8; TICKET_LENGTH] {
    let full = full_hash(data);
    full[..TICKET_LENGTH].try_into().unwrap()
}

fn pack_u64_msgpack(value: u64) -> Result<Vec<u8>, LXMFError> {
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &Value::Integer(value.into()))?;
    Ok(buf)
}

fn stamp_workblock(material: &[u8], expand_rounds: usize) -> Result<Vec<u8>, LXMFError> {
    let mut workblock = Vec::with_capacity(expand_rounds * 256);
    for n in 0..expand_rounds {
        let mut salt_material = Vec::with_capacity(material.len() + 16);
        salt_material.extend_from_slice(material);
        salt_material.extend_from_slice(&pack_u64_msgpack(n as u64)?);
        let salt = full_hash(&salt_material);
        let hk = Hkdf::<Sha256>::new(Some(&salt), material);
        let mut out = [0u8; 256];
        hk.expand(&[], &mut out).map_err(|_| LXMFError::InvalidPayload)?;
        workblock.extend_from_slice(&out);
    }
    Ok(workblock)
}

fn stamp_value(workblock: &[u8], stamp: &[u8]) -> u32 {
    let mut data = Vec::with_capacity(workblock.len() + stamp.len());
    data.extend_from_slice(workblock);
    data.extend_from_slice(stamp);
    let hash = full_hash(&data);

    let mut count = 0u32;
    for byte in hash {
        if byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }

    count
}

fn stamp_valid(workblock: &[u8], stamp: &[u8], target_cost: u32) -> bool {
    if target_cost == 0 {
        return true;
    }
    if target_cost > 256 {
        return false;
    }

    let mut data = Vec::with_capacity(workblock.len() + stamp.len());
    data.extend_from_slice(workblock);
    data.extend_from_slice(stamp);
    let hash = full_hash(&data);

    let shift = 256u32.saturating_sub(target_cost) as usize;
    if shift >= 256 {
        return true;
    }

    let mut target = [0u8; HASH_LENGTH];
    let byte_index = (HASH_LENGTH - 1) - (shift / 8);
    let bit_index = shift % 8;
    target[byte_index] = 1u8 << bit_index;

    for (hash_byte, target_byte) in hash.iter().zip(target.iter()) {
        if hash_byte < target_byte {
            return true;
        }
        if hash_byte > target_byte {
            return false;
        }
    }

    true
}

fn generate_stamp(
    message_id: &[u8; HASH_LENGTH],
    stamp_cost: u32,
    expand_rounds: usize,
) -> (Option<[u8; STAMP_SIZE]>, u32) {
    let workblock = match stamp_workblock(message_id, expand_rounds) {
        Ok(wb) => wb,
        Err(_) => return (None, 0),
    };

    let mut stamp = [0u8; STAMP_SIZE];
    let mut rounds = 0u32;
    
    // Limit rounds to prevent infinite loops in tests
    // For cost 18, typical rounds needed: ~262k, but can be much higher
    // Use a reasonable limit: 10M rounds (should be enough for cost 18, but prevents infinite loops)
    let max_rounds = if stamp_cost <= 15 {
        u32::MAX // Allow unlimited for low costs
    } else if stamp_cost <= 20 {
        10_000_000u32 // 10M rounds for medium costs (should be enough for cost 18)
    } else {
        1_000_000u32 // 1M rounds for high costs
    };

    loop {
        OsRng.fill_bytes(&mut stamp);
        rounds = rounds.saturating_add(1);
        if stamp_valid(&workblock, &stamp, stamp_cost) {
            let value = stamp_value(&workblock, &stamp);
            return (Some(stamp), value);
        }

        if rounds >= max_rounds {
            return (None, 0);
        }
    }
}

pub fn validate_peering_key(peering_id: &[u8], peering_key: &[u8], target_cost: u32) -> bool {
    let workblock = match stamp_workblock(peering_id, WORKBLOCK_EXPAND_ROUNDS_PEERING) {
        Ok(wb) => wb,
        Err(_) => return false,
    };
    stamp_valid(&workblock, peering_key, target_cost)
}

#[cfg(feature = "rns")]
pub fn generate_peering_key(
    peering_id: &[u8; HASH_LENGTH],
    target_cost: u32,
) -> Option<Vec<u8>> {
    let (stamp, _value) =
        generate_stamp(peering_id, target_cost, WORKBLOCK_EXPAND_ROUNDS_PEERING);
    stamp.map(|stamp| stamp.to_vec())
}

pub fn validate_pn_stamp(
    transient_data: &[u8],
    target_cost: u32,
) -> Option<([u8; HASH_LENGTH], Vec<u8>, u32, Vec<u8>)> {
    if transient_data.len() <= LXMF_OVERHEAD + STAMP_SIZE {
        return None;
    }

    let split_at = transient_data.len().saturating_sub(STAMP_SIZE);
    let (lxm_data, stamp) = transient_data.split_at(split_at);
    let transient_id = full_hash(lxm_data);
    let workblock = stamp_workblock(&transient_id, WORKBLOCK_EXPAND_ROUNDS_PN).ok()?;

    if !stamp_valid(&workblock, stamp, target_cost) {
        return None;
    }

    let value = stamp_value(&workblock, stamp);
    Some((transient_id, lxm_data.to_vec(), value, stamp.to_vec()))
}

fn build_signed_part(
    destination_hash: &[u8; DESTINATION_LENGTH],
    source_hash: &[u8; DESTINATION_LENGTH],
    packed_payload: &[u8],
    message_id: &[u8; 32],
) -> Vec<u8> {
    let mut signed_part = Vec::with_capacity(
        DESTINATION_LENGTH * 2 + packed_payload.len() + message_id.len(),
    );
    signed_part.extend_from_slice(destination_hash);
    signed_part.extend_from_slice(source_hash);
    signed_part.extend_from_slice(packed_payload);
    signed_part.extend_from_slice(message_id);
    signed_part
}

fn value_to_bytes(value: &Value) -> Result<Vec<u8>, LXMFError> {
    match value {
        Value::Binary(data) => Ok(data.clone()),
        Value::String(s) => s
            .as_str()
            .map(|v| v.as_bytes().to_vec())
            .ok_or(LXMFError::InvalidBytes),
        _ => Err(LXMFError::InvalidBytes),
    }
}

fn value_to_f64(value: &Value) -> Result<f64, LXMFError> {
    match value {
        Value::F64(v) => Ok(*v),
        Value::F32(v) => Ok(f64::from(*v)),
        Value::Integer(i) => i.as_i64().map(|v| v as f64).ok_or(LXMFError::InvalidTimestamp),
        _ => Err(LXMFError::InvalidTimestamp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workblock_matches_python_vector() {
        let material = hex::decode(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .expect("material hex");
        let workblock = stamp_workblock(&material, 130).expect("workblock");
        assert_eq!(workblock.len(), 130 * 256);

        let hash = full_hash(&workblock);
        assert_eq!(
            hex::encode(hash),
            "2cac1448698eadabe0312f3f682d32e99fff37ae720dfd94b15e47e5b8edd47d"
        );
    }

    #[test]
    fn stamp_value_matches_python_vector() {
        let material = hex::decode(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .expect("material hex");
        let workblock = stamp_workblock(&material, 130).expect("workblock");
        let stamp = hex::decode(
            "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
        )
        .expect("stamp hex");

        let value = stamp_value(&workblock, &stamp);
        assert_eq!(value, 2);
        assert!(stamp_valid(&workblock, &stamp, 2));
        assert!(!stamp_valid(&workblock, &stamp, 3));
    }

    #[test]
    fn validate_peering_key_matches_stamp_rules() {
        let peering_id = [0x11u8; HASH_LENGTH];
        let target_cost = 2;
        let (stamp, _) = generate_stamp(&peering_id, target_cost, WORKBLOCK_EXPAND_ROUNDS_PEERING);
        let stamp = stamp.expect("stamp");
        assert!(validate_peering_key(&peering_id, &stamp, target_cost));

        let workblock = stamp_workblock(&peering_id, WORKBLOCK_EXPAND_ROUNDS_PEERING).expect("workblock");
        let value = stamp_value(&workblock, &stamp);
        assert!(!validate_peering_key(&peering_id, &stamp, value.saturating_add(1)));
    }

    #[test]
    fn validate_pn_stamp_round_trip() {
        let lxm_len = LXMF_OVERHEAD + 1;
        let lxm_data = vec![0x55u8; lxm_len];
        let transient_id = full_hash(&lxm_data);
        let target_cost = 2;
        let (stamp, _) = generate_stamp(&transient_id, target_cost, WORKBLOCK_EXPAND_ROUNDS_PN);
        let stamp = stamp.expect("stamp");
        let mut transient_data = lxm_data.clone();
        transient_data.extend_from_slice(&stamp);

        let (out_id, out_lxm, out_value, out_stamp) =
            validate_pn_stamp(&transient_data, target_cost).expect("valid pn stamp");
        assert_eq!(out_id, transient_id);
        assert_eq!(out_lxm, lxm_data);
        assert_eq!(out_stamp, stamp.to_vec());

        let workblock = stamp_workblock(&transient_id, WORKBLOCK_EXPAND_ROUNDS_PN).expect("workblock");
        let expected_value = stamp_value(&workblock, &stamp);
        assert_eq!(out_value, expected_value);

        let too_short = vec![0x00u8; LXMF_OVERHEAD + STAMP_SIZE];
        assert!(validate_pn_stamp(&too_short, target_cost).is_none());
    }
}
