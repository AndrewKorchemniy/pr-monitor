use super::*;

#[test]
fn whats_new_links_the_tag_of_a_release_and_the_list_otherwise() {
    assert_eq!(
        whats_new_url("0.3.0"),
        "https://github.com/AndrewKorchemniy/pr-monitor/releases/tag/v0.3.0"
    );
    assert_eq!(
        whats_new_url("0.3.0-dev"),
        "https://github.com/AndrewKorchemniy/pr-monitor/releases"
    );
}
