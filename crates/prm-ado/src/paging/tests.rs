use http::HeaderValue;

use super::*;

#[test]
fn skip_pager_short_page_continues_until_empty() {
    let mut pager = SkipPager::new(10);
    let (items, step) = pager.accept(vec![1, 2, 3], |x| *x);
    assert_eq!((items.len(), step), (3, PageStep::Continue));
    assert_eq!(pager.skip(), 3);
    let (items, step) = pager.accept(Vec::<i32>::new(), |x| *x);
    assert_eq!((items.len(), step), (0, PageStep::Done));
}

#[test]
fn skip_pager_advances_by_returned_count() {
    let mut pager = SkipPager::new(10);
    pager.accept(vec![1, 2], |x| *x);
    assert_eq!(pager.skip(), 2);
    pager.accept(vec![3], |x| *x);
    assert_eq!(pager.skip(), 3);
}

#[test]
fn skip_pager_dedupes() {
    let mut pager = SkipPager::new(10);
    pager.accept(vec![1, 2], |x| *x);
    let (items, _) = pager.accept(vec![2, 3], |x| *x);
    assert_eq!(items, vec![3]);
}

#[test]
fn skip_pager_stops_when_skip_ignored() {
    let mut pager = SkipPager::new(10);
    pager.accept(vec![1, 2], |x| *x);
    let (items, step) = pager.accept(vec![1, 2], |x| *x);
    assert_eq!((items, step), (Vec::<i32>::new(), PageStep::Done));
}

#[test]
fn skip_pager_max_pages_truncated() {
    let mut pager = SkipPager::new(2);
    pager.accept(vec![1], |x| *x);
    let (_, step) = pager.accept(vec![2], |x| *x);
    assert_eq!(step, PageStep::Truncated);
}

fn with_token(token: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(H_CONTINUATION, HeaderValue::from_static(token));
    headers
}

#[test]
fn continuation_guard_repeated_token_stops() {
    let mut guard = ContinuationGuard::default();
    assert_eq!(guard.next(&with_token("abc")), Some("abc".to_owned()));
    assert_eq!(guard.next(&with_token("abc")), None);
}

#[test]
fn continuation_guard_missing_or_empty_stops() {
    let mut guard = ContinuationGuard::default();
    assert_eq!(guard.next(&HeaderMap::new()), None);
    assert_eq!(guard.next(&with_token(" ")), None);
}
