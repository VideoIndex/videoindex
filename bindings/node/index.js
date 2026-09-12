"use strict";
// Public entry: wraps the native module so `ask` is an AsyncIterable and
// options are plain objects.
const native = require("./native.js");

class Index {
  /** @param {import('./native').Index} inner */
  constructor(inner) {
    this._inner = inner;
  }
  /** Open an existing index directory. */
  static open(path, opts) {
    return new Index(native.Index.open(path, opts));
  }
  /** Create an empty index directory. */
  static create(path, opts) {
    return new Index(native.Index.create(path, opts));
  }
  get path() {
    return this._inner.path;
  }
  videos() {
    return this._inner.videos();
  }
  status() {
    return this._inner.status();
  }
  timeline(videoId, level) {
    return this._inner.timeline(videoId, level);
  }
  search(query, opts) {
    return this._inner.search(query, opts);
  }
  add(source, policy, force) {
    return this._inner.add(source, policy, force);
  }
  /** Ask a question; iterate the returned AsyncIterable of events, or `await .collect()`. */
  ask(question, opts) {
    const stream = this._inner.ask(question, opts);
    const iterable = {
      [Symbol.asyncIterator]() {
        return {
          async next() {
            const value = await stream.next();
            return value == null ? { value: undefined, done: true } : { value, done: false };
          },
        };
      },
      /** Drain the stream into `{ text, citations, toolCalls, usage, partial }`. */
      async collect() {
        const out = { text: "", citations: [], toolCalls: [], usage: null, partial: false, reason: null };
        for await (const ev of iterable) {
          if (ev.type === "token") out.text += ev.text;
          else if (ev.type === "citation") out.citations.push(ev);
          else if (ev.type === "tool_call") out.toolCalls.push(ev);
          else if (ev.type === "done") {
            out.usage = ev.usage;
            out.partial = ev.partial;
            out.reason = ev.reason;
          }
        }
        return out;
      },
    };
    return iterable;
  }
}

module.exports = { Index, version: native.version };
