/*! Integration tests for Eidetica.
 *
 * This test suite is organized as a single integration test binary
 * following the pattern described by matklad in
 * https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html
 *
 * The module structure mirrors the main library structure:
 * - atomicop: Tests for the AtomicOp struct and its interaction with EntryBuilder
 * - auth_integration: Tests for the authentication integration features
 * - basedb: Tests for the BaseDB struct and related functionality
 * - backend: Tests for the Backend trait and implementations
 * - data: Tests for the CRDT trait and implementations (e.g., KVOverWrite)
 * - entry: Tests for the Entry struct and related functionality
 * - tree: Tests for the Tree struct and related functionality
 */

mod atomicop;
mod auth_integration;
mod backend;
mod basedb;
mod cache;
mod data;
mod entry;
mod helpers;
mod subtree;
mod tree;
