use super::{RANKS, Rank};

#[test]
fn ranks_hand_out_room_in_declaration_order() {
    for (i, rank) in RANKS.into_iter().enumerate() {
        assert_eq!(rank as usize, i, "{rank:?}");
    }
    assert_eq!(RANKS.len(), Rank::Repos as usize + 1);
}
