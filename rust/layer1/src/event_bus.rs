//! 事件总线模块
//!
//! 发布订阅、事件溯源、持久化。

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 全局 handler ID 计数器
static HANDLER_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Handler ID 用于取消订阅
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(usize);

/// 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    pub payload: String,
    pub timestamp: String,
}

/// 事件处理器（带 ID）
struct HandlerEntry {
    id: HandlerId,
    handler: Box<dyn Fn(&Event) + Send + Sync>,
}

/// 事件总线
pub struct EventBus {
    handlers: RwLock<HashMap<String, Vec<HandlerEntry>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// 订阅事件，返回 handler ID 用于取消订阅
    pub fn subscribe<F>(&self, event_type: &str, handler: F) -> HandlerId
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        let id = HandlerId(HANDLER_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
        let entry = HandlerEntry {
            id,
            handler: Box::new(handler),
        };
        self.handlers
            .write()
            .entry(event_type.to_string())
            .or_default()
            .push(entry);
        id
    }

    /// 取消订阅
    pub fn unsubscribe(&self, event_type: &str, handler_id: HandlerId) -> bool {
        let mut handlers = self.handlers.write();

        // 获取条目，执行 retain，并记录结果
        let (removed, is_empty) = if let Some(entries) = handlers.get_mut(event_type) {
            let original_len = entries.len();
            entries.retain(|e| e.id != handler_id);
            let new_len = entries.len();
            (original_len > new_len, new_len == 0)
        } else {
            return false;
        };

        // 现在可以安全地移除 key（entries 的借用已结束）
        if is_empty {
            handlers.remove(event_type);
        }

        removed
    }

    /// 取消某事件类型的所有订阅
    pub fn unsubscribe_all(&self, event_type: &str) -> usize {
        let mut handlers = self.handlers.write();
        handlers.remove(event_type).map(|v| v.len()).unwrap_or(0)
    }

    /// 发布事件
    pub fn publish(&self, event: &Event) {
        if let Some(handlers) = self.handlers.read().get(&event.event_type) {
            for entry in handlers {
                (entry.handler)(event);
            }
        }
    }

    /// 获取某事件类型的订阅数量
    pub fn subscriber_count(&self, event_type: &str) -> usize {
        self.handlers
            .read()
            .get(event_type)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// 获取所有事件类型的订阅总数
    pub fn total_subscribers(&self) -> usize {
        self.handlers.read().values().map(|v| v.len()).sum()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_subscribe_and_publish() {
        let bus = EventBus::new();
        let received = Arc::new(Mutex::new(String::new()));
        let received_clone = Arc::clone(&received);

        bus.subscribe("test_event", move |event| {
            *received_clone.lock().unwrap() = event.payload.clone();
        });

        let event = Event {
            id: "1".to_string(),
            event_type: "test_event".to_string(),
            payload: "hello world".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        bus.publish(&event);

        assert_eq!(*received.lock().unwrap(), "hello world");
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);
        let counter2 = Arc::clone(&counter);

        bus.subscribe("increment", move |_| {
            *counter1.lock().unwrap() += 1;
        });

        bus.subscribe("increment", move |_| {
            *counter2.lock().unwrap() += 10;
        });

        let event = Event {
            id: "1".to_string(),
            event_type: "increment".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        bus.publish(&event);

        assert_eq!(*counter.lock().unwrap(), 11);
    }

    #[test]
    fn test_no_subscribers() {
        let bus = EventBus::new();

        let event = Event {
            id: "1".to_string(),
            event_type: "unknown_event".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        // 应该不崩溃
        bus.publish(&event);
    }

    #[test]
    fn test_different_event_types() {
        let bus = EventBus::new();
        let results = Arc::new(Mutex::new(Vec::new()));
        let r1 = Arc::clone(&results);
        let r2 = Arc::clone(&results);

        bus.subscribe("event_a", move |_| {
            r1.lock().unwrap().push("A");
        });

        bus.subscribe("event_b", move |_| {
            r2.lock().unwrap().push("B");
        });

        let event_a = Event {
            id: "1".to_string(),
            event_type: "event_a".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        let event_b = Event {
            id: "2".to_string(),
            event_type: "event_b".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        bus.publish(&event_a);
        bus.publish(&event_b);

        let res = results.lock().unwrap();
        assert_eq!(*res, vec!["A", "B"]);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            id: "123".to_string(),
            event_type: "test".to_string(),
            payload: "data".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("123"));
        assert!(json.contains("test"));
        assert!(json.contains("data"));
    }

    #[test]
    fn test_event_deserialization() {
        let json = r#"{
            "id": "456",
            "event_type": "my_event",
            "payload": "my_payload",
            "timestamp": "2024-01-01T00:00:00Z"
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.id, "456");
        assert_eq!(event.event_type, "my_event");
        assert_eq!(event.payload, "my_payload");
    }

    #[test]
    fn test_default_event_bus() {
        let bus = EventBus::default();
        let event = Event {
            id: "1".to_string(),
            event_type: "test".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        bus.publish(&event); // 应该不崩溃
    }

    #[test]
    fn test_concurrent_publish() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::thread;

        let bus = Arc::new(EventBus::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = Arc::clone(&counter);
        bus.subscribe("count", move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        });

        let mut handles = vec![];
        for _ in 0..10 {
            let b = Arc::clone(&bus);
            handles.push(thread::spawn(move || {
                let event = Event {
                    id: "1".to_string(),
                    event_type: "count".to_string(),
                    payload: String::new(),
                    timestamp: String::new(),
                };
                b.publish(&event);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_event_with_empty_payload() {
        let bus = EventBus::new();
        let received = Arc::new(Mutex::new(false));
        let r = Arc::clone(&received);

        bus.subscribe("empty", move |_| {
            *r.lock().unwrap() = true;
        });

        let event = Event {
            id: "1".to_string(),
            event_type: "empty".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        bus.publish(&event);

        assert!(*received.lock().unwrap());
    }

    #[test]
    fn test_unsubscribe() {
        let bus = EventBus::new();
        let counter = Arc::new(Mutex::new(0));
        let c1 = Arc::clone(&counter);

        let handler_id = bus.subscribe("test", move |_| {
            *c1.lock().unwrap() += 1;
        });

        assert_eq!(bus.subscriber_count("test"), 1);

        let event = Event {
            id: "1".to_string(),
            event_type: "test".to_string(),
            payload: String::new(),
            timestamp: String::new(),
        };

        bus.publish(&event);
        assert_eq!(*counter.lock().unwrap(), 1);

        // 取消订阅
        assert!(bus.unsubscribe("test", handler_id));
        assert_eq!(bus.subscriber_count("test"), 0);

        // 再次发布不应该触发
        bus.publish(&event);
        assert_eq!(*counter.lock().unwrap(), 1);

        // 重复取消应该返回 false
        assert!(!bus.unsubscribe("test", handler_id));
    }

    #[test]
    fn test_unsubscribe_all() {
        let bus = EventBus::new();

        bus.subscribe("a", |_| {});
        bus.subscribe("a", |_| {});
        bus.subscribe("b", |_| {});

        assert_eq!(bus.total_subscribers(), 3);

        let removed = bus.unsubscribe_all("a");
        assert_eq!(removed, 2);
        assert_eq!(bus.total_subscribers(), 1);
        assert_eq!(bus.subscriber_count("a"), 0);
        assert_eq!(bus.subscriber_count("b"), 1);
    }

    #[test]
    fn test_handler_id_unique() {
        let bus = EventBus::new();
        let id1 = bus.subscribe("test", |_| {});
        let id2 = bus.subscribe("test", |_| {});
        assert_ne!(id1, id2);
    }
}
