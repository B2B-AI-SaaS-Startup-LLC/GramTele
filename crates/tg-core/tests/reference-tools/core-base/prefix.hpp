#pragma once
#include "base/basic_types.h"
#include "base/flat_set.h"
#include "base/algorithm.h"
#include "rpl/rpl.h"
#include <range/v3/all.hpp>
#include <iostream>
// Only strong-id adapters: all ordering and list algorithms are compiled from
// unmodified storage_sparse_ids_list.cpp. MsgId.bare is int64 in the client.
using MsgId = int64;
constexpr auto ServerMaxMsgId = MsgId(1LL << 56);
struct MsgRange { MsgId from = 0; MsgId till = 0; friend bool operator==(const MsgRange&,const MsgRange&)=default; };
using UserId=uint64; using PhotoId=uint64;
#include <deque>
