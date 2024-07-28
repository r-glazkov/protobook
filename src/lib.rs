mod proto {
    include!(concat!(env!("OUT_DIR"), "/protobook.rs"));
}
#[cfg(feature = "fb2")]
mod fb2;

pub use proto::*;

impl AsRef<str> for link::Href {
    fn as_ref(&self) -> &str {
        match self {
            link::Href::Remote(url) => url,
            link::Href::Local(id) => id,
        }
    }
}
