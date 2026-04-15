//! PTY manager module.
//!
//! Owns all PTY state on a dedicated thread. Communicates with the rest
//! of the application via channel-based messages (PtyRequest / PtyResponse).

// Implementation in Task 1B.
