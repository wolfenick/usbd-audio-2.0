# usbd-audio-2.0

**This crate has not been published to `crates.io` and is currently in development.**

Audio 2.0 USB class implementation for [`usb-device`](https://crates.io/crates/usb-device).

This crate is a derivation of [`usbd-audio`](https://github.com/kiffie/usbd-audio) modified to implement the Audio 2.0 specification with implicit feedback synchronisation.

The USB descriptor from `usbd-audio-2.0` can exceed the default buffer size; enabling the `control-buffer-256` feature of the `usb-device` crate may be required.