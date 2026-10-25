//! M0: a recorded session (tests/fixtures/mux/session1.raw, made by record.sh) replays correctly through the terminal-state crate.
fn feed(chunk: usize) -> vt100::Parser {
    let raw = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/mux/session1.raw")).unwrap();
    let mut p = super::parser(24, 80, 1000);
    for c in raw.chunks(chunk) { p.process(c); } // any chunking (also inside an escape or a UTF-8 character) gives the same screen
    p
}

#[test]
fn replay_screen() {
    let p = feed(usize::MAX);
    let s = p.screen();
    let rows: Vec<String> = s.rows(0, 80).collect();
    assert_eq!(rows.len(), 24);
    assert_eq!(rows[23], "done");
    assert!(!s.alternate_screen(), "left the alt screen");
    assert!(rows.iter().all(|r| !r.contains("alt screen")), "alt screen content is gone");
    assert!(rows.iter().any(|r| r == "scrolled"), "scroll region output: {:?}", rows);
    assert!(rows.iter().any(|r| r == "line30"), "{:?}", rows);
}

#[test]
fn replay_attributes_and_wide() {
    // first two lines were cleared by 2J, so replay only the head of the stream
    let raw = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/mux/session1.raw")).unwrap();
    let head = &raw[..raw.windows(4).position(|w| w == b"\x1b[2J").unwrap()];
    let mut p = super::parser(24, 80, 0);
    p.process(head);
    let s = p.screen();
    let c = s.cell(0, 0).unwrap();
    assert!(c.bold() && c.fgcolor() == vt100::Color::Idx(1));
    let t = s.cell(0, 9 + 1 + 1).unwrap(); // "red bold " is 9 wide, then truecolor
    assert_eq!(t.fgcolor(), vt100::Color::Rgb(10, 200, 30));
    assert_eq!(s.rows(0, 80).nth(1).unwrap(), "wide: 日本語 ok");
    assert!(s.cell(1, 6).unwrap().is_wide());
    assert!(s.cell(1, 7).unwrap().is_wide_continuation());
}

#[test]
fn replay_chunking_is_irrelevant() {
    assert_eq!(feed(usize::MAX).screen().contents(), feed(1).screen().contents());
    assert_eq!(feed(usize::MAX).screen().contents_formatted(), feed(7).screen().contents_formatted());
}

#[test]
fn diff_reproduces_screen() {
    // the stream of screen deltas the server sends to a client: applying them to a second parser gives the same screen
    let raw = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/mux/session1.raw")).unwrap();
    let mut server = super::parser(24, 80, 0);
    let mut client = super::parser(24, 80, 0);
    let mut prev = server.screen().clone();
    for c in raw.chunks(50) {
        server.process(c);
        client.process(server.screen().contents_diff(&prev).as_bytes());
        prev = server.screen().clone();
    }
    assert_eq!(server.screen().contents_formatted(), client.screen().contents_formatted());
}
