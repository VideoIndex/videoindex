//! `vi-pipeline`: turns a Source into a populated Index.
//!
//! - [`operator`]: the [`Operator`] trait, the [`Item`]s that flow between
//!   operators, and [`OpContext`].
//! - [`dag`]: derives the execution graph from operators' declared inputs and
//!   outputs; adding an operator never touches the scheduler.
//! - [`scheduler`]: runs the DAG as tokio tasks joined by bounded channels,
//!   checkpoints every stage, emits progress events.
//! - [`ops`]: the shipped operators (`subtitle_import`, `sample`, `phash`,
//!   `thumbnail`, `vad`, `asr`, ...).

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod callback;
pub mod dag;
pub mod operator;
pub mod ops;
pub mod scheduler;

pub use dag::Dag;
pub use operator::{
    Budget, CostEstimate, Emitter, FrameItem, InputSummary, Item, ItemKind, MediaItem, OpContext,
    OpInput, OpOutput, Operator, SpeechItem, StageFailures,
};
pub use scheduler::{JobOptions, JobReport, OperatorFactory, Scheduler};
