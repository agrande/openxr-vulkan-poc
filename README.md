This repo is a proof of concept for using OpenXR with Vulkan in rust.
I use it to test the implementation of XR within gfx.

# Current state
At this point, the code creates an XR session properly (as the logs seem to indicate), but is not starting the session yet (this should be straightforward) and is missing a render loop with something to render (less straightforward).
The vulkan part is roughly based on that tutorial: https://github.com/unknownue/vulkan-tutorial-rust/tree/master/src/tutorials

# Build target
I've only tested this on Oculus Quest. But it should be straightforward to adapt to another target such as PCVR (shouldn't need much more than adding a `main.rs`).

I'm compiling it for android with a patched version of cargo-apk (see PR: https://github.com/rust-windowing/android-ndk-rs/pull/138), in order to add the OpenXR loader library to the APK.
You need to download Oculus' OpenXR loader from their developper website to be able to test it on the Oculus Quest, and place it in a `runtime_libs` folder.
Then compile with `cargo-apk run --features vulkan,vr`.
