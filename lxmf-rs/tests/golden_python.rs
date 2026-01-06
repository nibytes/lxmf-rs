use ed25519_dalek::SigningKey;
use lxmf_rs::{LXMessage, Value};
use rmpv::Value as RmpValue;
use std::io::Cursor;

#[test]
fn matches_python_packed_bytes() {
    let dest_hash = hex::decode("fae321c442e3c9bdcd7a3e79d850e03c").unwrap();
    let src_hash = hex::decode("cf0b2a4a8d2a0b6978b71290da7cc80e").unwrap();

    let dest_hash: [u8; 16] = dest_hash.as_slice().try_into().unwrap();
    let src_hash: [u8; 16] = src_hash.as_slice().try_into().unwrap();

    let signing_key_bytes = hex::decode("606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f").unwrap();
    let signing_key = SigningKey::from_bytes(signing_key_bytes.as_slice().try_into().unwrap());

    let fields = Value::Map(vec![
        (Value::Integer(1.into()), Value::Binary(b"one".to_vec())),
        (Value::Integer(2.into()), Value::Integer(2.into())),
    ]);

    let mut msg = LXMessage::with_strings(dest_hash, src_hash, 1_700_000_000.0, "hi", "hello", fields);
    let expected = hex::decode(
        "fae321c442e3c9bdcd7a3e79d850e03c\
         cf0b2a4a8d2a0b6978b71290da7cc80e\
         7e5c67ef9a442c75c1dad8b57464c827e798f8c7fe6e9e51ab8abb0d7d227eef\
         b62569334182b0cc5b6e3dff7962d643fafee2db97eb6a0142f4c9d986820c0e\
         94cb41d954fc40000000c4026869c40568656c6c6f8201c4036f6e650202",
    )
    .unwrap();

    let encoded = msg.encode_signed(&signing_key).unwrap();
    assert_eq!(encoded, expected);

    let mut decoded = LXMessage::decode(&expected).unwrap();
    let reencoded = decoded.encode_with_signature().unwrap();
    assert_eq!(reencoded, expected);
}

#[test]
fn matches_python_packed_bytes_with_ticket_stamp() {
    let dest_hash = hex::decode("fae321c442e3c9bdcd7a3e79d850e03c").unwrap();
    let src_hash = hex::decode("cf0b2a4a8d2a0b6978b71290da7cc80e").unwrap();

    let dest_hash: [u8; 16] = dest_hash.as_slice().try_into().unwrap();
    let src_hash: [u8; 16] = src_hash.as_slice().try_into().unwrap();

    let signing_key_bytes = hex::decode("606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f").unwrap();
    let signing_key = SigningKey::from_bytes(signing_key_bytes.as_slice().try_into().unwrap());

    let fields = Value::Map(vec![
        (Value::Integer(1.into()), Value::Binary(b"one".to_vec())),
        (Value::Integer(2.into()), Value::Integer(2.into())),
    ]);

    let mut msg = LXMessage::with_strings(dest_hash, src_hash, 1_700_000_000.0, "hi", "hello", fields);
    msg.set_outbound_ticket((0u8..16).collect());
    msg.set_defer_stamp(false);

    let expected = hex::decode(
        "fae321c442e3c9bdcd7a3e79d850e03c\
         cf0b2a4a8d2a0b6978b71290da7cc80e\
         7e5c67ef9a442c75c1dad8b57464c827e798f8c7fe6e9e51ab8abb0d7d227eef\
         b62569334182b0cc5b6e3dff7962d643fafee2db97eb6a0142f4c9d986820c0e\
         95cb41d954fc40000000c4026869c40568656c6c6f8201c4036f6e650202\
         c4106c461d63b5c3383383759bd6804d7085",
    )
    .unwrap();

    let encoded = msg.encode_signed(&signing_key).unwrap();
    assert_eq!(encoded, expected);

    let mut decoded = LXMessage::decode(&expected).unwrap();
    let reencoded = decoded.encode_with_signature().unwrap();
    assert_eq!(reencoded, expected);
}

#[test]
fn matches_python_propagation_packed() {
    let dest_hash = hex::decode("fae321c442e3c9bdcd7a3e79d850e03c").unwrap();
    let src_hash = hex::decode("cf0b2a4a8d2a0b6978b71290da7cc80e").unwrap();

    let dest_hash: [u8; 16] = dest_hash.as_slice().try_into().unwrap();
    let src_hash: [u8; 16] = src_hash.as_slice().try_into().unwrap();

    let fields = Value::Map(vec![
        (Value::Integer(1.into()), Value::Binary(b"one".to_vec())),
        (Value::Integer(2.into()), Value::Integer(2.into())),
    ]);

    let msg = LXMessage::with_strings(dest_hash, src_hash, 1_700_000_000.0, "hi", "hello", fields);

    let expected = hex::decode(
        "92cb41da5700db814d4b91c4d0fae321c442e3c9bdcd7a3e79d850e03c1388a26b\
         f781106c6ac3ecb82cc68fc798d6e3186e170940a563ba276a83c2095c5061770\
         76a986169667a978cffebdae95a4e3201c1ed0550bb75ab672fce5fb10b739a8\
         8bba954484938d6f361e5845bb4b972bfaa25ac153f2d3bfc7b157a39c7b109ec\
         d2c9a7cb23315bb07983a38f5f5df593631fdf3da7a4919bce645114cee1fdb14\
         85c58baafc56351ddd8812d68b5c3a5e2184f4eb1ca3f5abe288d2e8d30719525\
         a671143a9025c0d1b189dfe6d008e88b78dfb02153d9bdccbd68",
    )
    .unwrap();

    let parsed = rmpv::decode::read_value(&mut Cursor::new(&expected)).unwrap();
    let arr = match parsed {
        RmpValue::Array(a) => a,
        _ => panic!("expected array"),
    };
    let timestamp = match &arr[0] {
        RmpValue::F64(v) => *v,
        _ => panic!("expected f64 timestamp"),
    };
    let lxm_data = match &arr[1] {
        RmpValue::Array(inner) => match &inner[0] {
            RmpValue::Binary(b) => b.clone(),
            _ => panic!("expected binary"),
        },
        _ => panic!("expected array"),
    };

    let encrypted_payload = &lxm_data[16..];
    let encoded = msg
        .propagation_packed_from_encrypted(encrypted_payload, timestamp, None)
        .unwrap();
    assert_eq!(encoded, expected);

    let transient = msg.transient_id_from_encrypted(encrypted_payload);
    assert_eq!(hex::encode(transient), "cc12cf4a8c37e6b936db9c43f78701e2370b887016bdbf6ad85975eaaacab293");
}

#[test]
fn matches_python_paper_packed_and_uri() {
    let dest_hash = hex::decode("fae321c442e3c9bdcd7a3e79d850e03c").unwrap();
    let src_hash = hex::decode("cf0b2a4a8d2a0b6978b71290da7cc80e").unwrap();

    let dest_hash: [u8; 16] = dest_hash.as_slice().try_into().unwrap();
    let src_hash: [u8; 16] = src_hash.as_slice().try_into().unwrap();

    let fields = Value::Map(vec![
        (Value::Integer(1.into()), Value::Binary(b"one".to_vec())),
        (Value::Integer(2.into()), Value::Integer(2.into())),
    ]);

    let msg = LXMessage::with_strings(dest_hash, src_hash, 1_700_000_000.0, "hi", "hello", fields);

    let expected_paper = hex::decode(
        "fae321c442e3c9bdcd7a3e79d850e03c4878346df79b648a21dbcf7eaf0675cbb\
         d2feeaf1b26d25bd2b6409ea86d6700a082c88568a157bb94d84230de1ed7e2a\
         124c9572edb87f0f1f22d73182d56fdff3ae4b3cd4e8b7f71aabcf57ae0b549cb\
         da92964d75bb833edf79ae487f0bc3588f76c8a797f9943d19c5a0185f3d3cdfb\
         6f7518b47b22a2840f72346a461d6ea7611b4f13859b8b39eac3dbfeddec19146\
         4f7dfe8dfa7cf1cc4c0a0d0a01cbb1094ac95e0766ac218e9c13b16ec50d08510\
         4a2f2088a8306c6dbbed394240f",
    )
    .unwrap();
    let expected_uri = "lxm://-uMhxELjyb3Nej552FDgPEh4NG33m2SKIdvPfq8Gdcu9L-6vGybSW9K2QJ6obWcAoILIhWihV7uU2EIw3h7X4qEkyVcu24fw8fItcxgtVv3_OuSzzU6Lf3GqvPV64LVJy9qSlk11u4M-33muSH8Lw1iPdsinl_mUPRnFoBhfPTzftvdRi0eyKihA9yNGpGHW6nYRtPE4Wbiznqw9v-3ewZFGT33-jfp88cxMCg0KAcuxCUrJXgdmrCGOnBOxbsUNCFEEovIIioMGxtu-05QkDw";

    let encrypted_payload = &expected_paper[16..];
    let paper_packed = msg.paper_packed_from_encrypted(encrypted_payload);
    assert_eq!(paper_packed, expected_paper);

    let uri = msg.as_uri_from_paper(&paper_packed);
    assert_eq!(uri, expected_uri);
}
