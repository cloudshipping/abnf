//! Rule Parsing.
//!
//! This module heavily relies on closures (and, by extension, functions) as
//! function arguments. There is two types of such closures: *parsing
//! closures* and *converting closures*.
//!
//! Parsing closures attempt to parse a value from the beginning of a buffer.
//! They can succeed, fail, or be undecided. Since the closures are given a
//! mutable reference to a the buffer, it is important that they follow some
//! rules. These are as follows: If the parsing closure succeeds, it must
//! drain the buffer to the end of whatever it successfully parsed. If the
//! parsing closure fails or is undecided, it must not drain anything from
//! the buffer. This is important for parsing closures that combine other
//! parsing closures: If an inner closure succeeds, it will drain the buffer.
//! If then a later inner closure fails leading to the entire outer closure
//! to fail, the outer closure needs to rewind to wherever it started. This
//! can be achieved by wrapping the entire closure inside the `group()`
//! function.
//!
//!
//! # Implementing Rules as ABNF Operators
//!
//! [RFC 5234] defines a number of operators. Here’s how these can be
//! implemented using this module.
//!
//! ## Concatenation: `Rule1 Rule2`
//!
//! Concatenation can be achieved simply by parsing one rule after another
//! returning early if a rule either fails or is undecided using the
//! `try_ready!()` macro. Since you are applying several rules, the new
//! rule needs to be wrapped in `group()`.
//!
//! For instance:
//!
//! ```
//! # #[macro_use] extern crate abnf;
//! # use abnf::{Async, BytesMut, Poll};
//! # use abnf::parse::rule::group;
//! # struct Res;
//! # struct E;
//! # fn rule1(buf: &mut BytesMut) -> Poll<Res, E> { Ok(Async::Ready(Res)) }
//! # fn rule2(buf: &mut BytesMut) -> Poll<Res, E> { Ok(Async::Ready(Res)) }
//! fn concat(buf: &mut BytesMut) -> Poll<Res, E> {
//!     group(buf, |buf| {
//!         try_ready!(rule1(buf));
//!         try_ready!(rule2(buf));
//!         Ok(Async::Ready(Res))
//!     })
//! }
//! # fn main() { }
//! ```
//!
//!
//! # Alternatives: `Rule1 / Rule2`
//!
//! Alternatives can be parses as a sequence of expressions producing an
//! optional result. The `try_opt!()` macro helps you with that: It returns
//! early on some result, not ready, or error. Make sure the inner expressions
//! rewind correctly.
//! 
//! ```
//! # #[macro_use] extern crate abnf;
//! # use abnf::{Async, BytesMut, Poll};
//! # use abnf::parse::rule::group;
//! # struct Res;
//! # struct E;
//! fn rule1(buf: &mut BytesMut) -> Poll<Option<Res>, E> {
//!     unimplemented!()
//! }
//!
//! fn rule2(buf: &mut BytesMut) -> Poll<Option<Res>, E> {
//!     unimplemented!()
//! }
//!
//! fn alt(buf: &mut BytesMut) -> Poll<Res, E> {
//!     try_opt!(rule1(buf));
//!     try_opt!(rule2(buf));
//!     Err(E)
//! }
//! # fn main() { }
//! ```
//! 
//!
//! # Optional Repetition: `*Rule`
//!
//! For optional repetition, `Rule` is parsed zero or more times. Generally,
//! when this happens you will want to parse each element and then do
//! something with it. This is what `repeat()` is for. It takes a closure
//! for element parsing and one for element processing. The latter also
//! also drives repetition by indicating whether more elements should be
//! parsed or a result returned.
//!
//! Here is an example applying a `rule()` as many times as it appears pushing
//! each returned value into a vec.
//!
//! ```
//! # #[macro_use] extern crate abnf;
//! # use abnf::{Async, BytesMut, Poll};
//! # use abnf::parse::rule::{group, repeat};
//! # struct Res;
//! # struct E;
//! # fn rule(buf: &mut BytesMut) -> Poll<Res, E> { Ok(Async::Ready(Res)) }
//! fn repeat_rule(buf: &mut BytesMut) -> Poll<Vec<Res>, E> {
//!     let mut res = Vec::new();
//!     try_ready!(repeat(buf, rule, |item| {
//!         match item {
//!             Ok(item) => {
//!                 res.push(item);
//!                 Ok(Async::NotReady)
//!             }
//!             Err(err) => Ok(Async::Ready(()))
//!         }
//!     }));
//!     Ok(Async::Ready(res))
//! }
//! # fn main() { }
//! ```
//!
//! # Specific and Limited Repititions: `<n>Rule` and `<a>*<b>Rule`
//!
//! Both of these happen relatively rarely on a rule-level, so there are no
//! special functions for them. Instead, you can use `repeat()` and pass a
//! counter into the `combine` closure.
//!
//! For instance, `6rule` could be implemented like so:
//!
//! ```
//! # #[macro_use] extern crate abnf;
//! # use abnf::{Async, BytesMut, Poll};
//! # use abnf::parse::rule::{group, repeat};
//! # struct Res;
//! # struct E;
//! # fn rule(buf: &mut BytesMut) -> Poll<Res, E> { Ok(Async::Ready(Res)) }
//! fn six_rule(buf: &mut BytesMut) -> Poll<Vec<Res>, E> {
//!     let mut res = Vec::new();
//!     let mut count = 0;
//!     try_ready!(repeat(buf, rule, |item| {
//!         count += 1;
//!         match item {
//!             Ok(item) => {
//!                 res.push(item);
//!                 if count == 6 {
//!                     Ok(Async::Ready(()))
//!                 }
//!                 else {
//!                     Ok(Async::NotReady)
//!                 }
//!             }
//!             Err(err) => Err(err)
//!         }
//!     }));
//!     Ok(Async::Ready(res))
//! }
//! # fn main() { }
//! ```
//!
//! # At Least Once Repetition: `1*Rule`
//!
//! For the variant of repetition where there needs to be at least on element,
//! there is a special function: `at_least_once()`. It works very much like
//! `repeat()` but fails if the `parse` closure fails on the first repetition.
//! In order to produce the correct error for this case, it takes yet another
//! closure.
//!
//! ```
//! # #[macro_use] extern crate abnf;
//! # use abnf::{Async, BytesMut, Poll};
//! # use abnf::parse::rule::{group, at_least_once};
//! # struct Res;
//! # struct E;
//! # fn rule(buf: &mut BytesMut) -> Poll<Res, E> { Ok(Async::Ready(Res)) }
//! fn rule_at_least_once(buf: &mut BytesMut) -> Poll<Vec<Res>, E> {
//!     let mut res = Vec::new();
//!     try_ready!(at_least_once(buf, rule,
//!         |item| {
//!             match item {
//!                 Ok(item) => {
//!                     res.push(item);
//!                     Ok(Async::NotReady)
//!                 }
//!                 Err(err) => Ok(Async::Ready(()))
//!             }
//!         },
//!         |_| E
//!     ));
//!     Ok(Async::Ready(res))
//! }
//! # fn main() { }
//! ```
//!
//!
//! ## Optional Sequence: `[RULE]`
//!
//! The `optional()` function serves the purpose of allowing a rule to be
//! applied at most once. It returns an `Option<R>`.
//!
//! So, say we want to parse this: `rule1 [rule2]`. This could look like
//! this:
//!
//! ```
//! # #[macro_use] extern crate abnf;
//! # use abnf::{Async, BytesMut, Poll};
//! # use abnf::parse::rule::{group, optional};
//! # struct Res1; struct Res2;
//! # struct E;
//! # fn rule1(buf: &mut BytesMut) -> Poll<Res1, E> { Ok(Async::Ready(Res1)) }
//! # fn rule2(buf: &mut BytesMut) -> Poll<Res2, E> { Ok(Async::Ready(Res2)) }
//! fn rule1_opt_rule2(buf: &mut BytesMut) -> Poll<(Res1, Option<Res2>), E> {
//!     group(buf, |buf| {
//!         let res1 = try_ready!(rule1(buf));
//!         let res2 = try_ready!(optional(buf, rule2));
//!         Ok(Async::Ready((res1, res2)))
//!     })
//! }
//! # fn main() { }
//! ```

use bytes::BytesMut;
use futures::{Async, Poll};


//------------ Combining Rules -----------------------------------------------

/// Succeeds if parsing within `op` succeeds or rewinds.
pub fn group<P, T, E>(buf: &mut BytesMut, parse: P) -> Poll<T, E>
           where P: FnOnce(&mut BytesMut) -> Poll<T, E> {
    let orig_buf = buf.clone();
    let res = parse(buf);
    match res {
        Ok(Async::NotReady) | Err(_) => *buf = orig_buf,
        _ => {}
    }
    res
}

pub fn opt_group<P, T, E>(buf: &mut BytesMut, parse: P) -> Poll<Option<T>, E>
                 where P: FnOnce(&mut BytesMut) -> Poll<Option<T>, E> {
    let orig_buf = buf.clone();
    let res = parse(buf);
    match res {
        Ok(Async::Ready(Some(_))) => { }
        _ => *buf = orig_buf,
    }
    res
}


/// Repetition.
///
/// This combinator is driven by two closures.
///
/// The first one, `parse`, parses an element at a time from the beginning
/// of the buffer given. If it returns non-ready, the whole repetition
/// rewinds and returns non-ready.
///
/// Otherwise, the `parse` closure’s result is transformed into a `Result`
/// and given to the closure `combine` which needs to decide what to do
/// next. If it returns an error, the whole repetition rewinds and results
/// in that error. It it returns a value, the repetition is over producing
/// this result. If it returns non-ready, another iterations is done.
pub fn repeat<P, R, E, C, S, F>(buf: &mut BytesMut, parse: P, mut combine: C)
                          -> Poll<S, F>
              where P: Fn(&mut BytesMut) -> Poll<R, E>,
                    C: FnMut(Result<R, E>) -> Poll<S, F> {
    group(buf, |buf| {
        loop {
            let item = try_result!(parse(buf));
            match combine(item) {
                Ok(Async::Ready(res)) => return Ok(Async::Ready(res)),
                Err(err) =>  return Err(err),
                Ok(Async::NotReady) => { }
            }
        }
    })
}


/// Repeat at least once.
///
/// This is like `repeat()`, but if `parse` fails already on the first time,
/// `combine` isn’t called at all but rather `empty`.
pub fn at_least_once<P, R, E, C, S, F, D>(buf: &mut BytesMut,
                                          parse: P, mut combine: C, error: D)
                                          -> Poll<S, F>
                     where P: Fn(&mut BytesMut) -> Poll<R, E>,
                           C: FnMut(Result<R, E>) -> Poll<S, F>,
                           D: FnOnce(E) -> F {
    group(buf, |buf| {
        match try_result!(parse(buf)) {
            Err(err) => return Err(error(err)),
            Ok(item) => match combine(Ok(item)) {
                Ok(Async::Ready(res)) => return Ok(Async::Ready(res)),
                Err(err) => return Err(err),
                Ok(Async::NotReady) => { }
            }
        }
        loop {
            let item = try_result!(parse(buf));
            match combine(item) {
                Ok(Async::Ready(res)) => return Ok(Async::Ready(res)),
                Err(err) =>  return Err(err),
                Ok(Async::NotReady) => { }
            }
        }
    })
}


/// An optional rule.
pub fn optional<P, R, E, F>(buf: &mut BytesMut, parse: P) -> Poll<Option<R>, F>
                where P: FnOnce(&mut BytesMut) -> Poll<R, E> {
    match parse(buf) {
        Ok(Async::NotReady) => Ok(Async::NotReady),
        Ok(Async::Ready(some)) => Ok(Async::Ready(Some(some))),
        Err(_) => Ok(Async::Ready(None))
    }
}

