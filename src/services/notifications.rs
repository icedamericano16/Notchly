//! Notification mirror via UserNotificationListener. Requires package identity
//! (MSIX/Store or a registered sparse package); degrades gracefully otherwise.

use std::collections::HashSet;
use std::time::Duration;

use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};

use super::{com_mta, spawn, Bus, Event};
use crate::util::log;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Allowed,
    Denied,
    /// The process has no package identity, so the API is unavailable.
    NoIdentity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u32,
    pub app: String,
    pub app_id: String,
    pub title: String,
    pub body: String,
}

pub fn has_identity() -> bool {
    windows::ApplicationModel::Package::Current().is_ok()
}

pub fn start(bus: Bus) {
    spawn("notifications", move || {
        com_mta();
        if !has_identity() {
            bus.send(Event::NotificationAccess(Access::NoIdentity));
            return;
        }
        let Ok(listener) = UserNotificationListener::Current() else {
            bus.send(Event::NotificationAccess(Access::NoIdentity));
            return;
        };
        let status = listener.RequestAccessAsync().and_then(|o| o.join());
        match status {
            Ok(UserNotificationListenerAccessStatus::Allowed) => {
                bus.send(Event::NotificationAccess(Access::Allowed))
            }
            Ok(_) => {
                bus.send(Event::NotificationAccess(Access::Denied));
                return;
            }
            Err(e) => {
                log(&format!("notification access failed: {e}"));
                bus.send(Event::NotificationAccess(Access::NoIdentity));
                return;
            }
        }
        // Prefer the NotificationChanged event; if this configuration refuses it
        // (it can require a background task), fall back to a cheap poll.
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let evented = listener
            .NotificationChanged(&windows::Foundation::TypedEventHandler::new(move |_, _| {
                let _ = tx.send(());
                Ok(())
            }))
            .is_ok();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut first = true;
        loop {
            if !first {
                if evented {
                    if rx.recv().is_err() {
                        return;
                    }
                    while rx.try_recv().is_ok() {}
                } else {
                    std::thread::sleep(Duration::from_millis(1500));
                }
            }
            if let Ok(list) = listener
                .GetNotificationsAsync(NotificationKinds::Toast)
                .and_then(|o| o.join())
            {
                let mut current = HashSet::new();
                for i in 0..list.Size().unwrap_or(0) {
                    let Ok(n) = list.GetAt(i) else { continue };
                    let id = n.Id().unwrap_or(0);
                    current.insert(id);
                    if seen.contains(&id) || first {
                        continue;
                    }
                    let app = n
                        .AppInfo()
                        .and_then(|a| a.DisplayInfo())
                        .and_then(|d| d.DisplayName())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let app_id = n
                        .AppInfo()
                        .and_then(|a| a.AppUserModelId())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let mut texts = Vec::new();
                    if let Ok(binding) = n.Notification().and_then(|t| t.Visual()).and_then(|v| {
                        v.GetBinding(&KnownNotificationBindings::ToastGeneric().unwrap_or_default())
                    }) {
                        if let Ok(els) = binding.GetTextElements() {
                            for j in 0..els.Size().unwrap_or(0) {
                                if let Ok(t) = els.GetAt(j).and_then(|e| e.Text()) {
                                    texts.push(t.to_string());
                                }
                            }
                        }
                    }
                    let title = texts.first().cloned().unwrap_or_else(|| app.clone());
                    let body = texts.iter().skip(1).cloned().collect::<Vec<_>>().join("\n");
                    bus.send(Event::Notification(Toast {
                        id,
                        app,
                        app_id,
                        title,
                        body,
                    }));
                }
                seen = current;
            }
            first = false;
        }
    });
}
