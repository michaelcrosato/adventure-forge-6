use forge_content::{compile_production, parse, parse_and_compile_production};
use forge_kernel::{
    Effect, EntropyError, EntropyState, EventKind, MAX_ENTROPY_CURSOR, canonical_json_bytes,
    enumerate_legal_actions, sha256_hex_bytes, sha256_json,
};
use forge_replay::{PlayerTrace, Session, resume_player_trace};
use serde_json::{Value, json};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");
const HASH_KILL: &str = "canonical hash lost ordered input";
const ENTROPY_KILL: &str = "entropy seed cursor sequence changed";

#[test]
fn canonical_hash_preserves_order_and_authoritative_inputs() {
    // These bytes and digest are literal answers, not another invocation of
    // the canonicalizer under test. Object order is irrelevant; array order
    // represents ordered action/effect programs and must remain significant.
    let left: Value = serde_json::from_str(
        r#"{"z":false,"route":["sluice","market"],"a":{"stock":3,"label":"tide\nkey"}}"#,
    )
    .unwrap();
    let right: Value = serde_json::from_str(
        r#"{"a":{"label":"tide\nkey","stock":3},"route":["sluice","market"],"z":false}"#,
    )
    .unwrap();
    let expected =
        br#"{"a":{"label":"tide\nkey","stock":3},"route":["sluice","market"],"z":false}"#;
    let expected_digest = "27c8dc80104836f03bc89e433855f1f32b0ff876b7ab40d59d9cd2abff27ef22";
    assert_eq!(
        canonical_json_bytes(&left).unwrap(),
        expected,
        "{HASH_KILL}"
    );
    assert_eq!(
        canonical_json_bytes(&right).unwrap(),
        expected,
        "{HASH_KILL}"
    );
    assert_eq!(sha256_json(&left).unwrap(), expected_digest, "{HASH_KILL}");
    assert_eq!(sha256_json(&right).unwrap(), expected_digest, "{HASH_KILL}");
    assert_eq!(
        sha256_hex_bytes(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "{HASH_KILL}"
    );

    let mut variants = Vec::new();
    let mut reordered = left.clone();
    reordered["route"].as_array_mut().unwrap().reverse();
    variants.push(reordered);
    for (pointer, replacement) in [
        ("/a/stock", json!(4)),
        ("/a/stock", json!("3")),
        ("/a/label", json!("tide key")),
        ("/z", json!(true)),
    ] {
        let mut changed = left.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        variants.push(changed);
    }
    let mut omitted = left.clone();
    omitted["a"].as_object_mut().unwrap().remove("stock");
    variants.push(omitted);
    for changed in variants {
        assert_ne!(
            sha256_json(&changed).unwrap(),
            expected_digest,
            "{HASH_KILL}"
        );
    }

    let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
    let initial = content.new_game("rook", 71).unwrap();
    let initial_hash = initial.state_id();
    let mut states = Vec::new();
    let mut changed = initial.clone();
    changed.entropy.seed += 1;
    states.push(changed);
    let mut changed = initial.clone();
    changed.entropy.cursor += 1;
    states.push(changed);
    let mut changed = initial.clone();
    changed.world.time += 1;
    states.push(changed);
    let mut changed = initial.clone();
    changed.character.resources.insert("coin".to_owned(), 99);
    states.push(changed);
    for changed in states {
        assert_ne!(changed.state_id(), initial_hash, "{HASH_KILL}");
    }
}

#[test]
fn entropy_known_answers_survive_canonical_actions_and_resume() {
    // Fixed SplitMix64 vectors include zero, neighboring, and full-width
    // seeds. Repeating the same faulty reducer cannot satisfy these answers.
    let vectors = [
        (
            0,
            [
                0xe220_a839_7b1d_cdaf,
                0x6e78_9e6a_a1b9_65f4,
                0x06c4_5d18_8009_454f,
                0xf88b_b8a8_724c_81ec,
            ],
        ),
        (
            1,
            [
                0x910a_2dec_8902_5cc1,
                0xbeeb_8da1_658e_ec67,
                0xf893_a2ee_fb32_555e,
                0x71c1_8690_ee42_c90b,
            ],
        ),
        (
            u64::MAX,
            [
                0xe4d9_7177_1b65_2c20,
                0xe99f_f867_dbf6_82c9,
                0x382f_f84c_b272_81e9,
                0x6d1d_b36c_cba9_82d2,
            ],
        ),
    ];
    for (seed, expected) in vectors {
        let mut entropy = EntropyState::new(seed);
        assert_eq!(entropy.algorithm, "splitmix64-v1", "{ENTROPY_KILL}");
        for (cursor, value) in expected.into_iter().enumerate() {
            let before = entropy.clone();
            let draw = entropy.draw().unwrap();
            assert_eq!(draw.value, value, "{ENTROPY_KILL}");
            assert_eq!(draw.before, before, "{ENTROPY_KILL}");
            assert_eq!(entropy, before, "{ENTROPY_KILL}");
            assert_eq!(draw.after.seed, seed, "{ENTROPY_KILL}");
            assert_eq!(draw.before.cursor, cursor as u64, "{ENTROPY_KILL}");
            assert_eq!(draw.after.cursor, cursor as u64 + 1, "{ENTROPY_KILL}");
            entropy = draw.after;
        }
    }
    let exhausted = EntropyState {
        cursor: MAX_ENTROPY_CURSOR,
        ..EntropyState::new(0)
    };
    assert_eq!(exhausted.draw(), Err(EntropyError::CursorExhausted));
    let unsupported = EntropyState {
        algorithm: "unsupported".to_owned(),
        ..EntropyState::new(0)
    };
    assert!(matches!(
        unsupported.draw(),
        Err(EntropyError::UnsupportedAlgorithm { .. })
    ));

    // This deliberately modified content is a random-effect fixture, not
    // evidence that the shipped Split Tide authors a chance-based action.
    // Retaining production genesis permits the real player-save boundary.
    let mut source = parse(SPLIT_TIDE).unwrap();
    source
        .actions
        .iter_mut()
        .find(|action| action.id == "wait_tide")
        .unwrap()
        .effects
        .insert(
            0,
            Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(Effect::AdjustResource {
                    resource: "coin".to_owned(),
                    amount: 1,
                }),
                on_failure: Box::new(Effect::AdjustResource {
                    resource: "coin".to_owned(),
                    amount: -1,
                }),
            },
        );
    let content = compile_production(source).unwrap();
    let mut session = Session::new_game("rook", 0, &content).unwrap();
    let initial_coins = session.state().character.resources["coin"];
    let mut checkpoint = None;
    for (cursor, (value, coin_delta)) in vectors[0].1.into_iter().zip([1, 2, 1, 2]).enumerate() {
        record_known_draw(&mut session, &content, cursor as u64, value);
        assert_eq!(
            session.state().character.resources["coin"],
            initial_coins + coin_delta,
            "{ENTROPY_KILL}"
        );
        if cursor == 1 {
            checkpoint = Some(session.player_trace().unwrap().to_json().unwrap());
        }
    }
    let saved = PlayerTrace::from_json(&checkpoint.unwrap()).unwrap();
    let mut resumed = resume_player_trace(&saved, &content).unwrap();
    for (cursor, value) in vectors[0].1.into_iter().enumerate().skip(2) {
        record_known_draw(&mut resumed, &content, cursor as u64, value);
    }
    assert_eq!(resumed.state(), session.state(), "{ENTROPY_KILL}");
    assert_eq!(resumed.trace(), session.trace(), "{ENTROPY_KILL}");
    assert_eq!(
        resumed.player_trace().unwrap(),
        session.player_trace().unwrap()
    );
}

fn record_known_draw(
    session: &mut Session<'_>,
    content: &forge_kernel::CompiledContent,
    cursor: u64,
    value: u64,
) {
    let action = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == "wait_tide" && action.parameters.is_empty())
        .unwrap();
    let recorded = session.record(&action).unwrap();
    assert_eq!(recorded.entropy_draws.len(), 1, "{ENTROPY_KILL}");
    assert_eq!(recorded.entropy_draws[0].value, value, "{ENTROPY_KILL}");
    assert_eq!(recorded.entropy_before.cursor, cursor, "{ENTROPY_KILL}");
    assert_eq!(recorded.entropy_after.cursor, cursor + 1, "{ENTROPY_KILL}");
    assert_eq!(session.state().entropy.cursor, cursor + 1, "{ENTROPY_KILL}");
    let draws: Vec<_> = recorded
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RandomDraw {
                algorithm,
                cursor,
                value,
            } => Some((algorithm.as_str(), *cursor, *value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        draws,
        vec![("splitmix64-v1", cursor, value)],
        "{ENTROPY_KILL}"
    );
}
