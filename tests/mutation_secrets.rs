use kanso::rules::redact;

#[test]
fn known_prefix_boundaries_and_entropy_gate() {
    for prefix in [
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        "xoxs-",
    ] {
        let short = format!("{prefix}aaaaaaa");
        let exact = format!("{prefix}aaaaaaaa");
        assert_eq!(redact(&short), short);
        assert_eq!(redact(&exact), "****aaaa");
    }
    for value in [
        "12345678901234567890123456789012",
        "abcdefghijklmnopqrstuvwxyzABCDEF",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1",
        "a1",
    ] {
        assert_eq!(redact(value), value);
    }
    assert_eq!(redact("Z8gP2mQ7vL4xN9aR6sT3wY5bC1dF0hKj"), "****0hKj");
}

#[test]
fn pem_blocks_and_adjacent_tokens_are_redacted_once() {
    let key = "-----BEGIN PRIVATE KEY-----\nabc123\n-----END PRIVATE KEY-----";
    let prefixed = format!("hi {key} tail ghp_abcdefgh");
    assert_eq!(redact(&prefixed), "hi ****---- tail ****efgh");
    let between = format!("ghp_abcdefgh {key} ghp_ijklmnop");
    assert_eq!(redact(&between), "****efgh ****---- ****mnop");
    let interior = "-----BEGIN PRIVATE KEY-----\nghp_abcdefgh\n-----END PRIVATE KEY-----";
    assert_eq!(redact(interior), "****----");
    let two = format!("text {key} then {key} done");
    assert_eq!(redact(&two), "text ****---- then ****---- done");
}

#[test]
fn nonprivate_headers_then_private_and_crlf() {
    let cert = "-----BEGIN CERTIFICATE-----\nbody\n-----END CERTIFICATE-----";
    let key = "-----BEGIN PRIVATE KEY-----\r\nbody\r\n-----END PRIVATE KEY-----";
    assert_eq!(
        redact(&format!("{cert} {key} done")),
        format!("{cert} ****---- done")
    );
    assert_eq!(redact("-----BEGIN PRIVATE KEY-----"), "****----");
}

#[test]
fn pem_scanning_terminates_for_private_and_nonprivate_headers() {
    use std::sync::mpsc;
    use std::time::Duration;

    for (input, expected) in [
        (
            "-----BEGIN PRIVATE KEY-----\nbody\n-----END PRIVATE KEY-----",
            "****----",
        ),
        (
            "-----BEGIN CERTIFICATE-----\nbody\n-----END CERTIFICATE-----",
            "-----BEGIN CERTIFICATE-----\nbody\n-----END CERTIFICATE-----",
        ),
    ] {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(redact(input));
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            expected
        );
    }
}
