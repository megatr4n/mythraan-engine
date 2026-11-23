use std::marker::PhantomData;

use super::{Access, Request, Requests, requests};

/// Matches entities that have the component `T`.
pub struct With<T>(PhantomData<fn() -> T>);

/// Matches entities that lack the component `T`.
pub struct Without<T>(PhantomData<fn() -> T>);

/// Matches entities whose component `T` was added within the query's tick range.
pub struct Added<T>(PhantomData<fn() -> T>);

/// Matches entities whose component `T` was added, replaced, or yielded
/// as a mutable reference within the query's tick range.
///
/// A mutable borrow counts as a change. Interior writes through shared
/// references are outside this tracking.
pub struct Changed<T>(PhantomData<fn() -> T>);

pub trait Condition {
    fn request() -> Request;
}
impl<T: Send + Sync + 'static> Condition for With<T> {
    fn request() -> Request {
        Request::of::<T>(Access::With)
    }
}
impl<T: Send + Sync + 'static> Condition for Without<T> {
    fn request() -> Request {
        Request::of::<T>(Access::Without)
    }
}
impl<T: Send + Sync + 'static> Condition for Added<T> {
    fn request() -> Request {
        Request::of::<T>(Access::Added)
    }
}
impl<T: Send + Sync + 'static> Condition for Changed<T> {
    fn request() -> Request {
        Request::of::<T>(Access::Changed)
    }
}

pub trait Filter {
    fn requests() -> Requests;
}
impl Filter for () {
    fn requests() -> Requests {
        requests([])
    }
}
impl<F: Condition> Filter for F {
    fn requests() -> Requests {
        requests([F::request()])
    }
}

macro_rules! tuple {
    ($($condition:ident),+) => {
        impl<$($condition: Condition),+> Filter for ($($condition,)+) {
            fn requests() -> Requests {
                requests([$($condition::request()),+])
            }
        }
    };
}
tuple!(A, B);
tuple!(A, B, C);
tuple!(A, B, C, D);
