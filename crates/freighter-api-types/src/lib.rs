#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "index")]
#[cfg_attr(docsrs, doc(cfg(feature = "index")))]
pub mod index;

#[cfg(feature = "auth")]
#[cfg_attr(docsrs, doc(cfg(feature = "auth")))]
pub mod auth;

#[cfg(feature = "ownership")]
#[cfg_attr(docsrs, doc(cfg(feature = "ownership")))]
pub mod ownership;

#[cfg(feature = "storage")]
#[cfg_attr(docsrs, doc(cfg(feature = "storage")))]
pub mod storage;
