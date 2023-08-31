use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use fxhash::FxHashSet;
use parking_lot::RwLock;

static SHARED_STRINGS: OnceLock<RwLock<FxHashSet<Arc<str>>>> = OnceLock::new();

pub trait ToSharedString {
    fn to_shared_string(self) -> Arc<str>;
}

fn get_shared_strings() -> &'static RwLock<FxHashSet<Arc<str>>> {
    SHARED_STRINGS.get_or_init(|| {
        rayon::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(60));
            SHARED_STRINGS
                .get()
                .unwrap()
                .write()
                .extract_if(|x| Arc::strong_count(x) == 1);
        });
        Default::default()
    })
}

impl ToSharedString for String {
    fn to_shared_string(self) -> Arc<str> {
        let map = get_shared_strings();
        {
            let reader = map.read();
            if let Some(shared) = reader.get(self.as_str()) {
                return shared.clone();
            }
        }
        let mut writer = map.write();
        let shared: Arc<str> = Arc::from(self.into_boxed_str());
        writer.insert(shared.clone());
        shared
    }
}

impl ToSharedString for &str {
    fn to_shared_string(self) -> Arc<str> {
        let map = get_shared_strings();
        {
            let reader = map.read();
            if let Some(shared) = reader.get(self) {
                return shared.clone();
            }
        }
        let mut writer = map.write();
        let shared: Arc<str> = Arc::from(self.to_string().into_boxed_str());
        writer.insert(shared.clone());
        shared
    }
}


impl ToSharedString for Arc<str> {
    fn to_shared_string(self) -> Arc<str> {
        self
    }
}