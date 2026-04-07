// This file is part of Gear.
//
// Copyright (C) 2024-2025 Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use jsonrpsee::types::ErrorObject;

// JSON-RPC server error codes (-32000..-32099 reserved for implementation).
// -32000/-32001 used by jsonrpsee itself, start at -32050.
const DB_ERROR: i32 = -32050;
const RUNTIME_ERROR: i32 = -32051;
const BAD_REQUEST_ERROR: i32 = -32052;
const INTERNAL_ERROR: i32 = -32053;

pub fn db(err: &'static str) -> ErrorObject<'static> {
    ErrorObject::owned(DB_ERROR, "Database error", Some(err))
}

pub fn runtime(err: impl ToString) -> ErrorObject<'static> {
    ErrorObject::owned(RUNTIME_ERROR, "Runtime error", Some(err.to_string()))
}

pub fn bad_request(err: impl ToString) -> ErrorObject<'static> {
    ErrorObject::owned(BAD_REQUEST_ERROR, "Bad request", Some(err.to_string()))
}

pub fn internal() -> ErrorObject<'static> {
    ErrorObject::owned(INTERNAL_ERROR, "Internal error", None::<&str>)
}
