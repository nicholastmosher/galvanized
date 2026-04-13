//! Async task definitions that may be deployed in this application
//!
//! The way I like to write async code is as a state machine over a
//! stream of events coming from input channels. I want to find a way
//! to slightly generalize this and integrate the GPUI/global-context
//! design pattern into it, but that will be an ongoing experiment
//! because I don't currently know exactly how that will work.

pub mod willow_task;
