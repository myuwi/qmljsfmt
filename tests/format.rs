use std::fs;

#[test]
fn formats_fixtures() {
    insta::glob!("fixtures/*.qml", |path| {
        let input = fs::read_to_string(path).unwrap();
        let formatted = qmljsfmt::format(&input).unwrap();

        assert_eq!(
            qmljsfmt::format(&formatted).unwrap(),
            formatted,
            "a second pass changed the result"
        );

        let before_and_after = format!("{input}=== formatted ===\n{formatted}");
        insta::assert_snapshot!(before_and_after);
    });
}
