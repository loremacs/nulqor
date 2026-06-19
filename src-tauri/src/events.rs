//! Namespace-scoped event bus (DESIGN.md §6, ADR 003).
//!
//! Events are notifications: fire-and-forget, many subscribers can receive each event.
//! The bus is indexed by namespace so only matching subscribers are woken — no broadcast-then-discard.
//!
//! Subscription IDs are stable u64s. Subscriptions persist until explicitly cancelled.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::error::CoreError;
use crate::types::{EventPattern, NamespacedEvent};

/// Opaque subscription handle. Call `EventBus::unsubscribe(id)` to cancel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u64);

struct Entry {
    id: u64,
    pattern: EventPattern,
    handler: Arc<dyn Fn(&NamespacedEvent) + Send + Sync>,
}

pub struct EventBus {
    /// Indexed by namespace so `publish` only touches relevant entries.
    subs: RwLock<HashMap<String, Vec<Entry>>>,
    next_id: AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subs: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Publish an event. Wakes only subscribers whose pattern matches `ev.id`.
    pub fn publish(&self, ev: NamespacedEvent) -> Result<(), CoreError> {
        let handlers: Vec<Arc<dyn Fn(&NamespacedEvent) + Send + Sync>> = {
            let subs = self.subs.read().unwrap_or_else(|e| e.into_inner());
            subs.get(&ev.id.namespace)
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|entry| entry.pattern.matches(&ev.id))
                        .map(|entry| Arc::clone(&entry.handler))
                        .collect()
                })
                .unwrap_or_default()
        };
        for handler in handlers {
            handler(&ev);
        }
        Ok(())
    }

    /// Subscribe to events matching `pattern`. Returns a stable id for later cancellation.
    pub fn subscribe(
        &self,
        pattern: EventPattern,
        handler: Arc<dyn Fn(&NamespacedEvent) + Send + Sync>,
    ) -> SubscriptionId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let entry = Entry { id, pattern: pattern.clone(), handler };
        let mut subs = self.subs.write().unwrap_or_else(|e| e.into_inner());
        subs.entry(pattern.namespace.clone()).or_default().push(entry);
        SubscriptionId(id)
    }

    /// Cancel a subscription by the id returned from `subscribe`.
    pub fn unsubscribe(&self, sub_id: SubscriptionId) {
        let mut subs = self.subs.write().unwrap_or_else(|e| e.into_inner());
        for entries in subs.values_mut() {
            entries.retain(|e| e.id != sub_id.0);
        }
    }

    /// Count active subscribers in a namespace (for testing).
    #[cfg(test)]
    fn subscriber_count(&self, namespace: &str) -> usize {
        self.subs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(namespace)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Tests — BUILD_PLAN §1.2
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventId;

    fn make_event(ns: &str, name: &str, version: u32) -> NamespacedEvent {
        NamespacedEvent {
            id: EventId { namespace: ns.into(), name: name.into(), version },
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn publish_delivers_to_matching_subscriber() {
        let bus = EventBus::new();
        let received = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = received.clone();
        bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(move |_| { r.store(true, Ordering::SeqCst); }),
        );
        bus.publish(make_event("canvas", "ready", 1)).unwrap();
        assert!(received.load(Ordering::SeqCst), "subscriber should have been called");
    }

    #[test]
    fn publish_does_not_wake_non_matching_namespace() {
        let bus = EventBus::new();
        let called = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = called.clone();
        bus.subscribe(
            EventPattern::namespace("canvas"),
            Arc::new(move |_| { c.fetch_add(1, Ordering::SeqCst); }),
        );
        // Publish to a DIFFERENT namespace — subscriber must NOT be woken
        bus.publish(make_event("provider", "reply-complete", 1)).unwrap();
        assert_eq!(called.load(Ordering::SeqCst), 0, "non-matching subscriber woken");
    }

    #[test]
    fn fifty_subscribers_only_matching_namespace_receives() {
        let bus = EventBus::new();
        let canvas_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let other_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

        // 25 canvas subscribers
        for _ in 0..25 {
            let c = canvas_count.clone();
            bus.subscribe(
                EventPattern::namespace("canvas"),
                Arc::new(move |_| { c.fetch_add(1, Ordering::SeqCst); }),
            );
        }
        // 25 provider subscribers
        for _ in 0..25 {
            let c = other_count.clone();
            bus.subscribe(
                EventPattern::namespace("provider"),
                Arc::new(move |_| { c.fetch_add(1, Ordering::SeqCst); }),
            );
        }

        // Publish ONE canvas event
        bus.publish(make_event("canvas", "ready", 1)).unwrap();

        assert_eq!(canvas_count.load(Ordering::SeqCst), 25, "all 25 canvas subs should fire");
        assert_eq!(other_count.load(Ordering::SeqCst), 0, "provider subs must NOT fire");
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let bus = EventBus::new();
        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = count.clone();
        let id = bus.subscribe(
            EventPattern::namespace("canvas"),
            Arc::new(move |_| { c.fetch_add(1, Ordering::SeqCst); }),
        );
        bus.publish(make_event("canvas", "ready", 1)).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);

        bus.unsubscribe(id);
        bus.publish(make_event("canvas", "ready", 1)).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1, "after unsubscribe no more calls");
    }

    #[test]
    fn namespace_filter_respects_event_name() {
        let bus = EventBus::new();
        let ready_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let r = ready_count.clone();
        bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(move |_| { r.fetch_add(1, Ordering::SeqCst); }),
        );
        // Different event name in same namespace — should NOT fire
        bus.publish(make_event("canvas", "layout-saved", 1)).unwrap();
        assert_eq!(ready_count.load(Ordering::SeqCst), 0);

        // Correct event — should fire
        bus.publish(make_event("canvas", "ready", 1)).unwrap();
        assert_eq!(ready_count.load(Ordering::SeqCst), 1);
    }
}
