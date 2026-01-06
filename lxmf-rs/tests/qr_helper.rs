use lxmf_rs::{LXMessage, Value};

#[test]
fn qr_helper_produces_non_empty_text() {
    let msg = LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "hello",
        Value::Map(vec![]),
    );

    let paper = hex::decode(
        "fae321c442e3c9bdcd7a3e79d850e03c4878346df79b648a21dbcf7eaf0675cbb\
         d2feeaf1b26d25bd2b6409ea86d6700a082c88568a157bb94d84230de1ed7e2a\
         124c9572edb87f0f1f22d73182d56fdff3ae4b3cd4e8b7f71aabcf57ae0b549cb\
         da92964d75bb833edf79ae487f0bc3588f76c8a797f9943d19c5a0185f3d3cdfb\
         6f7518b47b22a2840f72346a461d6ea7611b4f13859b8b39eac3dbfeddec19146\
         4f7dfe8dfa7cf1cc4c0a0d0a01cbb1094ac95e0766ac218e9c13b16ec50d08510\
         4a2f2088a8306c6dbbed394240f",
    )
    .unwrap();

    let qr = msg.as_qr_from_paper(&paper).unwrap();
    assert!(!qr.is_empty());
    assert!(qr.contains('█'));
}
