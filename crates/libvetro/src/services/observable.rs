//! A small observable value for the UI thread: the current value, callbacks, and an async
//! stream of changes. Single-threaded by design (`Rc`, no locks); clones share the value.

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};

type Callback<T> = Rc<RefCell<dyn FnMut(&T)>>;

struct Inner<T> {
    value: RefCell<T>,
    version: Cell<u64>,
    next_id: Cell<u64>,
    callbacks: RefCell<Vec<(u64, Callback<T>)>>,
    wakers: RefCell<Vec<Waker>>,
}

/// A value that tells who watches it when it changes. `set` ignores equal values.
pub struct Observable<T> {
    inner: Rc<Inner<T>>,
}

impl<T> Clone for Observable<T> {
    fn clone(&self) -> Self {
        Observable {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T: std::fmt::Debug + Clone + PartialEq + 'static> std::fmt::Debug for Observable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Observable").field(&self.get()).finish()
    }
}

impl<T: Clone + PartialEq + 'static> Observable<T> {
    pub fn new(value: T) -> Self {
        Observable {
            inner: Rc::new(Inner {
                value: RefCell::new(value),
                version: Cell::new(0),
                next_id: Cell::new(0),
                callbacks: RefCell::new(Vec::new()),
                wakers: RefCell::new(Vec::new()),
            }),
        }
    }

    /// The current value.
    pub fn get(&self) -> T {
        self.inner.value.borrow().clone()
    }

    /// Replaces the value; returns whether it changed (watchers run only then).
    pub fn set(&self, value: T) -> bool {
        {
            let Ok(mut current) = self.inner.value.try_borrow_mut() else {
                return false;
            };
            if *current == value {
                return false;
            }
            *current = value;
        }
        self.notify();
        true
    }

    /// Changes the value in place; returns whether it changed.
    pub fn update(&self, change: impl FnOnce(&mut T)) -> bool {
        let mut next = self.get();
        change(&mut next);
        self.set(next)
    }

    /// Calls `callback` after every change (not for the current value). The returned
    /// [`Subscription`] stops it when dropped.
    pub fn subscribe(&self, callback: impl FnMut(&T) + 'static) -> Subscription {
        let id = self.inner.next_id.get();
        self.inner.next_id.set(id.wrapping_add(1));
        let callback: Callback<T> = Rc::new(RefCell::new(callback));
        self.inner.callbacks.borrow_mut().push((id, callback));
        let weak: Weak<Inner<T>> = Rc::downgrade(&self.inner);
        Subscription {
            cancel: Some(Box::new(move || {
                if let Some(inner) = weak.upgrade()
                    && let Ok(mut list) = inner.callbacks.try_borrow_mut()
                {
                    list.retain(|(i, _)| *i != id);
                }
            })),
        }
    }

    /// Like [`subscribe`](Self::subscribe), and calls `callback` once now with the current value.
    pub fn observe(&self, mut callback: impl FnMut(&T) + 'static) -> Subscription {
        callback(&self.get());
        self.subscribe(callback)
    }

    /// A stream of the values that follow, starting after the current one. Changes that come
    /// faster than the stream is polled are merged: it yields the latest value.
    pub fn changes(&self) -> Changes<T> {
        Changes {
            inner: Rc::clone(&self.inner),
            seen: self.inner.version.get(),
        }
    }

    fn notify(&self) {
        self.inner
            .version
            .set(self.inner.version.get().wrapping_add(1));
        let value = self.get();
        let callbacks: Vec<Callback<T>> = self
            .inner
            .callbacks
            .borrow()
            .iter()
            .map(|(_, c)| Rc::clone(c))
            .collect();
        for callback in callbacks {
            // A callback that sets the value again is already running: skip, do not panic.
            if let Ok(mut callback) = callback.try_borrow_mut() {
                callback(&value);
            }
        }
        let wakers: Vec<Waker> = self.inner.wakers.borrow_mut().drain(..).collect();
        for waker in wakers {
            waker.wake();
        }
    }
}

impl<T: Clone + PartialEq + Default + 'static> Default for Observable<T> {
    fn default() -> Self {
        Observable::new(T::default())
    }
}

/// Keeps a callback registered; dropping it unregisters.
#[must_use = "dropping the subscription stops the callback"]
pub struct Subscription {
    cancel: Option<Box<dyn FnOnce()>>,
}

impl Subscription {
    /// Keeps the callback for as long as the observable lives.
    pub fn keep(mut self) {
        self.cancel = None;
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel();
        }
    }
}

/// The changes of an [`Observable`] as an async stream (see [`Observable::changes`]). It holds
/// the value alive and never ends by itself.
pub struct Changes<T> {
    inner: Rc<Inner<T>>,
    seen: u64,
}

impl<T: Clone> Changes<T> {
    /// Polls for the next value.
    pub fn poll_next_value(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        let version = self.inner.version.get();
        if version != self.seen {
            self.seen = version;
            return Poll::Ready(self.inner.value.borrow().clone());
        }
        if let Ok(mut wakers) = self.inner.wakers.try_borrow_mut()
            && !wakers.iter().any(|w| w.will_wake(cx.waker()))
        {
            wakers.push(cx.waker().clone());
        }
        Poll::Pending
    }

    /// Waits for the next change.
    pub async fn next(&mut self) -> T {
        std::future::poll_fn(|cx| self.poll_next_value(cx)).await
    }
}

impl<T: Clone> futures_lite::Stream for Changes<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.poll_next_value(cx).map(Some)
    }
}

impl<T> Unpin for Changes<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::{block_on, or};

    #[test]
    fn set_notifies_only_on_change() {
        let value = Observable::new(1);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let _sub = value.subscribe(move |v| log.borrow_mut().push(*v));
        assert!(!value.set(1));
        assert!(value.set(2));
        assert!(value.update(|v| *v += 1));
        assert_eq!(*seen.borrow(), vec![2, 3]);
        assert_eq!(value.get(), 3);
    }

    #[test]
    fn observe_calls_now_and_drop_unsubscribes() {
        let value = Observable::new("a".to_owned());
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let sub = value.observe(move |v| log.borrow_mut().push(v.clone()));
        value.set("b".into());
        drop(sub);
        value.set("c".into());
        assert_eq!(*seen.borrow(), vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn reentrant_set_does_not_panic() {
        let value = Observable::new(0);
        let again = value.clone();
        value
            .subscribe(move |v| {
                again.set(*v + 100);
            })
            .keep();
        value.set(1);
        assert_eq!(value.get(), 101);
    }

    #[test]
    fn changes_yield_the_latest_value() {
        let value = Observable::new(0);
        let mut changes = value.changes();
        value.set(1);
        value.set(2);
        assert_eq!(block_on(changes.next()), 2);
        let other = value.clone();
        let got = block_on(or(changes.next(), async move {
            other.set(5);
            std::future::pending().await
        }));
        assert_eq!(got, 5);
    }
}
