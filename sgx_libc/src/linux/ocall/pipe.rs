// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License..

use crate::linux::*;
use sgx_oc::linux::ocall;

#[no_mangle]
pub unsafe extern "C" fn pipe2(fds: *mut [c_int; 2], flags: c_int) -> c_int {
    if fds.is_null() {
        set_errno(EINVAL);
        return -1;
    }

    if ocall::pipe2(&mut *fds, flags).is_ok() {
        0
    } else {
        -1
    }
}