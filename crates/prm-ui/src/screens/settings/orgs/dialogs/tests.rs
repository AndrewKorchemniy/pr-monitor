use super::{Block, reflow};

#[test]
fn hard_wrapped_paragraphs_join_and_rules_and_lists_stand_alone() {
    let text = "Copyright notice that was
wrapped by hand.

-----
Terms:
1) first
  indented
- item";
    assert_eq!(
        reflow(text),
        [
            Block::Text("Copyright notice that was wrapped by hand.".to_owned()),
            Block::Rule,
            Block::Text(
                "Terms:
1) first
  indented
- item"
                    .to_owned()
            ),
        ]
    );
}
