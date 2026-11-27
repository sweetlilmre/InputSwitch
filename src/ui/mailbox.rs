//! A window's mailbox: messages for it from anywhere (other windows, worker threads), applied
//! when the window gets to them. The window is woken with [`WM_MAIL`]; it takes what's waiting
//! with [`Mailbox::take`] once it's free, so nothing is lost while it's busy (a dialog is open).
//!
//! Each time the window opens, the mailbox starts a new generation: work started for an earlier
//! window posts with [`Mailbox::post_for`] and is dropped if that window has gone.

use crate::ffi::*;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

/// Sent to a window when mail arrives.
pub const WM_MAIL: u32 = WM_APP + 2;

pub struct Mailbox<T> {
    items: Mutex<Vec<T>>,
    window: AtomicIsize,
    generation: AtomicU32,
}

impl<T> Mailbox<T> {
    pub const fn new() -> Self {
        Mailbox { items: Mutex::new(Vec::new()), window: AtomicIsize::new(0), generation: AtomicU32::new(0) }
    }

    /// Delivers to `window` from now on, starting a new generation (anything waiting is dropped).
    pub fn open(&self, window: HWND) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.items().clear();
        self.window.store(window, Ordering::Relaxed);
    }

    /// The window has gone: mail is dropped until it opens again.
    pub fn close(&self) {
        self.window.store(0, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.items().clear();
    }

    /// The current generation, for work that might outlive the window (see [`Mailbox::post_for`]).
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn post(&self, item: T) {
        self.post_for(self.generation(), item);
    }

    /// Posts `item` if the window that was open in `generation` is still open.
    pub fn post_for(&self, generation: u32, item: T) {
        let window = self.window.load(Ordering::Relaxed);
        if window == 0 || generation != self.generation() {
            return;
        }
        self.items().push(item);
        self.wake();
    }

    /// Everything waiting, oldest first.
    pub fn take(&self) -> Vec<T> {
        std::mem::take(&mut *self.items())
    }

    /// Wakes the window again if mail is still waiting (it arrived while the window was busy).
    pub fn wake_if_waiting(&self) {
        if !self.items().is_empty() {
            self.wake();
        }
    }

    fn wake(&self) {
        unsafe { PostMessageW(self.window.load(Ordering::Relaxed), WM_MAIL, 0, 0) };
    }

    fn items(&self) -> std::sync::MutexGuard<'_, Vec<T>> {
        self.items.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
