use super::*;

#[test]
fn vote_kinds() {
    let table = [
        (10, VoteKind::Approved),
        (5, VoteKind::ApprovedWithSuggestions),
        (0, VoteKind::NoVote),
        (-5, VoteKind::WaitingForAuthor),
        (-10, VoteKind::Rejected),
        (15, VoteKind::NotApplicable),
        (3, VoteKind::Other),
    ];
    for (v, k) in table {
        assert_eq!(Vote(v).kind(), k, "{v}");
    }
}
