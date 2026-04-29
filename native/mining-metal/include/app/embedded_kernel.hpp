#pragma once

namespace app {

inline constexpr const char kArgon2MetalKernelSource[] = R"METAL(
#include <metal_stdlib>
using namespace metal;

#define ARGON2_BLOCK_SIZE 1024
#define ARGON2_QWORDS_IN_BLOCK (ARGON2_BLOCK_SIZE / 8)
#define ARGON2_SYNC_POINTS 4
#define THREADS_PER_LANE 32
#define QWORDS_PER_THREAD (ARGON2_QWORDS_IN_BLOCK / THREADS_PER_LANE)
#define ARGON2_REFS_PER_BLOCK (ARGON2_BLOCK_SIZE / (2 * sizeof(uint)))

struct u64_shuffle_buf {
    uint lo[THREADS_PER_LANE];
    uint hi[THREADS_PER_LANE];
};

struct block_g {
    ulong data[ARGON2_QWORDS_IN_BLOCK];
};

struct block_th {
    ulong a;
    ulong b;
    ulong c;
    ulong d;
};

struct ref_entry {
    uint ref_index;
    uint ref_lane;
};

struct precompute_args {
    uint passes;
    uint lanes;
    uint segment_blocks;
};

struct oneshot_args {
    uint passes;
    uint lanes;
    uint segment_blocks;
};

struct segment_args {
    uint passes;
    uint lanes;
    uint segment_blocks;
    uint pass;
    uint slice;
};

inline ulong u64_build(uint hi, uint lo) {
    return (((ulong)hi) << 32) | (ulong)lo;
}

inline uint u64_lo(ulong x) {
    return (uint)x;
}

inline uint u64_hi(ulong x) {
    return (uint)(x >> 32);
}

inline ulong rotr64(ulong x, uint n) {
    return (x >> n) | (x << (64 - n));
}

inline uint mul_hi_u32(uint x, uint y) {
    return mulhi(x, y);
}

inline ulong u64_shuffle(ulong v, uint thread_src, uint thread_id,
                         threadgroup u64_shuffle_buf *buf) {
    uint lo = u64_lo(v);
    uint hi = u64_hi(v);
    buf->lo[thread_id] = lo;
    buf->hi[thread_id] = hi;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    lo = buf->lo[thread_src];
    hi = buf->hi[thread_src];
    return u64_build(hi, lo);
}

inline ulong cmpeq_mask(uint test, uint ref) {
    uint x = -(uint)(test == ref);
    return u64_build(x, x);
}

inline ulong block_th_get(thread const block_th *b, uint idx) {
    ulong res = 0;
    res ^= cmpeq_mask(idx, 0) & b->a;
    res ^= cmpeq_mask(idx, 1) & b->b;
    res ^= cmpeq_mask(idx, 2) & b->c;
    res ^= cmpeq_mask(idx, 3) & b->d;
    return res;
}

inline void block_th_set(thread block_th *b, uint idx, ulong v) {
    b->a ^= cmpeq_mask(idx, 0) & (v ^ b->a);
    b->b ^= cmpeq_mask(idx, 1) & (v ^ b->b);
    b->c ^= cmpeq_mask(idx, 2) & (v ^ b->c);
    b->d ^= cmpeq_mask(idx, 3) & (v ^ b->d);
}

inline void move_block(thread block_th *dst, thread const block_th *src) {
    *dst = *src;
}

inline void xor_block(thread block_th *dst, thread const block_th *src) {
    dst->a ^= src->a;
    dst->b ^= src->b;
    dst->c ^= src->c;
    dst->d ^= src->d;
}

inline void load_block(thread block_th *dst, device const block_g *src, uint thread_id) {
    dst->a = src->data[0 * THREADS_PER_LANE + thread_id];
    dst->b = src->data[1 * THREADS_PER_LANE + thread_id];
    dst->c = src->data[2 * THREADS_PER_LANE + thread_id];
    dst->d = src->data[3 * THREADS_PER_LANE + thread_id];
}

inline void load_block_xor(thread block_th *dst, device const block_g *src, uint thread_id) {
    dst->a ^= src->data[0 * THREADS_PER_LANE + thread_id];
    dst->b ^= src->data[1 * THREADS_PER_LANE + thread_id];
    dst->c ^= src->data[2 * THREADS_PER_LANE + thread_id];
    dst->d ^= src->data[3 * THREADS_PER_LANE + thread_id];
}

inline void store_block(device block_g *dst, thread const block_th *src, uint thread_id) {
    dst->data[0 * THREADS_PER_LANE + thread_id] = src->a;
    dst->data[1 * THREADS_PER_LANE + thread_id] = src->b;
    dst->data[2 * THREADS_PER_LANE + thread_id] = src->c;
    dst->data[3 * THREADS_PER_LANE + thread_id] = src->d;
}

inline ulong f(ulong x, ulong y) {
    uint xlo = u64_lo(x);
    uint ylo = u64_lo(y);
    return x + y + 2 * u64_build(mul_hi_u32(xlo, ylo), xlo * ylo);
}

inline void g(thread block_th *block) {
    ulong a = block->a;
    ulong b = block->b;
    ulong c = block->c;
    ulong d = block->d;
    a = f(a, b);
    d = rotr64(d ^ a, 32);
    c = f(c, d);
    b = rotr64(b ^ c, 24);
    a = f(a, b);
    d = rotr64(d ^ a, 16);
    c = f(c, d);
    b = rotr64(b ^ c, 63);
    block->a = a;
    block->b = b;
    block->c = c;
    block->d = d;
}

inline uint apply_shuffle_shift1(uint thread_id, uint idx) {
    return (thread_id & 0x1c) | ((thread_id + idx) & 0x3);
}

inline uint apply_shuffle_unshift1(uint thread_id, uint idx) {
    idx = (QWORDS_PER_THREAD - idx) % QWORDS_PER_THREAD;
    return apply_shuffle_shift1(thread_id, idx);
}

inline uint apply_shuffle_shift2(uint thread_id, uint idx) {
    uint lo = (thread_id & 0x1) | ((thread_id & 0x10) >> 3);
    lo = (lo + idx) & 0x3;
    return ((lo & 0x2) << 3) | (thread_id & 0xe) | (lo & 0x1);
}

inline uint apply_shuffle_unshift2(uint thread_id, uint idx) {
    idx = (QWORDS_PER_THREAD - idx) % QWORDS_PER_THREAD;
    return apply_shuffle_shift2(thread_id, idx);
}

inline void shuffle_shift1(thread block_th *block, uint thread_id,
                           threadgroup u64_shuffle_buf *buf) {
    for (uint i = 0; i < QWORDS_PER_THREAD; i++) {
        uint src_thr = apply_shuffle_shift1(thread_id, i);
        ulong v = block_th_get(block, i);
        v = u64_shuffle(v, src_thr, thread_id, buf);
        block_th_set(block, i, v);
    }
}

inline void shuffle_unshift1(thread block_th *block, uint thread_id,
                             threadgroup u64_shuffle_buf *buf) {
    for (uint i = 0; i < QWORDS_PER_THREAD; i++) {
        uint src_thr = apply_shuffle_unshift1(thread_id, i);
        ulong v = block_th_get(block, i);
        v = u64_shuffle(v, src_thr, thread_id, buf);
        block_th_set(block, i, v);
    }
}

inline void shuffle_shift2(thread block_th *block, uint thread_id,
                           threadgroup u64_shuffle_buf *buf) {
    for (uint i = 0; i < QWORDS_PER_THREAD; i++) {
        uint src_thr = apply_shuffle_shift2(thread_id, i);
        ulong v = block_th_get(block, i);
        v = u64_shuffle(v, src_thr, thread_id, buf);
        block_th_set(block, i, v);
    }
}

inline void shuffle_unshift2(thread block_th *block, uint thread_id,
                             threadgroup u64_shuffle_buf *buf) {
    for (uint i = 0; i < QWORDS_PER_THREAD; i++) {
        uint src_thr = apply_shuffle_unshift2(thread_id, i);
        ulong v = block_th_get(block, i);
        v = u64_shuffle(v, src_thr, thread_id, buf);
        block_th_set(block, i, v);
    }
}

inline void transpose(thread block_th *block, uint thread_id,
                      threadgroup u64_shuffle_buf *buf) {
    uint thread_group = (thread_id & 0x0C) >> 2;
    for (uint i = 1; i < QWORDS_PER_THREAD; i++) {
        uint thr = (i << 2) ^ thread_id;
        uint idx = thread_group ^ i;
        ulong v = block_th_get(block, idx);
        v = u64_shuffle(v, thr, thread_id, buf);
        block_th_set(block, idx, v);
    }
}

inline void shuffle_block(thread block_th *block, uint thread_id,
                          threadgroup u64_shuffle_buf *buf) {
    transpose(block, thread_id, buf);
    g(block);
    shuffle_shift1(block, thread_id, buf);
    g(block);
    shuffle_unshift1(block, thread_id, buf);
    transpose(block, thread_id, buf);
    g(block);
    shuffle_shift2(block, thread_id, buf);
    g(block);
    shuffle_unshift2(block, thread_id, buf);
}

inline void compute_ref_pos(uint lanes, uint segment_blocks,
                            uint pass, uint lane, uint slice, uint offset,
                            thread uint *ref_lane, thread uint *ref_index) {
    uint lane_blocks = ARGON2_SYNC_POINTS * segment_blocks;
    *ref_lane = *ref_lane % lanes;

    uint base;
    if (pass != 0) {
        base = lane_blocks - segment_blocks;
    } else {
        if (slice == 0) {
            *ref_lane = lane;
        }
        base = slice * segment_blocks;
    }

    uint ref_area_size = base + offset - 1;
    if (*ref_lane != lane) {
        ref_area_size = min(ref_area_size, base);
    }

    *ref_index = mul_hi_u32(*ref_index, *ref_index);
    *ref_index = ref_area_size - 1 - mul_hi_u32(ref_area_size, *ref_index);

    if (pass != 0 && slice != ARGON2_SYNC_POINTS - 1) {
        *ref_index += (slice + 1) * segment_blocks;
        if (*ref_index >= lane_blocks) {
            *ref_index -= lane_blocks;
        }
    }
}

inline void argon2_core(device block_g *memory, device block_g *mem_curr,
                        thread block_th *prev, thread block_th *tmp,
                        threadgroup u64_shuffle_buf *shuffle_buf, uint lanes,
                        uint thread_id, uint pass, uint ref_index, uint ref_lane) {
    device block_g *mem_ref = memory + ref_index * lanes + ref_lane;
    if (pass != 0) {
        load_block(tmp, mem_curr, thread_id);
        load_block_xor(prev, mem_ref, thread_id);
        xor_block(tmp, prev);
    } else {
        load_block_xor(prev, mem_ref, thread_id);
        move_block(tmp, prev);
    }
    shuffle_block(prev, thread_id, shuffle_buf);
    xor_block(prev, tmp);
    store_block(mem_curr, prev, thread_id);
}

inline void next_addresses(thread block_th *addr, thread block_th *tmp,
                           uint thread_input, uint thread_id,
                           threadgroup u64_shuffle_buf *buf) {
    addr->a = u64_build(0, thread_input);
    addr->b = 0;
    addr->c = 0;
    addr->d = 0;
    shuffle_block(addr, thread_id, buf);
    addr->a ^= u64_build(0, thread_input);
    move_block(tmp, addr);
    shuffle_block(addr, thread_id, buf);
    xor_block(addr, tmp);
}

inline void argon2_step_precompute(device block_g *memory, device block_g *mem_curr,
                                   thread block_th *prev, thread block_th *tmp,
                                   threadgroup u64_shuffle_buf *shuffle_buf,
                                   device const ref_entry *refs,
                                   thread uint *refs_offset,
                                   uint lanes, uint segment_blocks, uint thread_id,
                                   uint lane, uint pass, uint slice, uint offset) {
    uint ref_index;
    uint ref_lane;
    bool data_independent = pass == 0 && slice < ARGON2_SYNC_POINTS / 2;
    if (data_independent) {
        const ref_entry ref = refs[*refs_offset];
        ref_index = ref.ref_index;
        ref_lane = ref.ref_lane;
        (*refs_offset)++;
    } else {
        ulong v = u64_shuffle(prev->a, 0, thread_id, shuffle_buf);
        ref_index = u64_lo(v);
        ref_lane = u64_hi(v);
        compute_ref_pos(lanes, segment_blocks, pass, lane, slice, offset, &ref_lane, &ref_index);
    }
    argon2_core(memory, mem_curr, prev, tmp, shuffle_buf, lanes, thread_id, pass, ref_index, ref_lane);
}

inline void argon2_step(device block_g *memory, device block_g *mem_curr,
                        thread block_th *prev, thread block_th *tmp, thread block_th *addr,
                        threadgroup u64_shuffle_buf *shuffle_buf,
                        uint lanes, uint segment_blocks, uint thread_id, thread uint *thread_input,
                        uint lane, uint pass, uint slice, uint offset) {
    uint ref_index;
    uint ref_lane;
    bool data_independent = pass == 0 && slice < ARGON2_SYNC_POINTS / 2;
    if (data_independent) {
        uint addr_index = offset % ARGON2_QWORDS_IN_BLOCK;
        if (addr_index == 0) {
            if (thread_id == 6) {
                ++*thread_input;
            }
            next_addresses(addr, tmp, *thread_input, thread_id, shuffle_buf);
        }
        uint thr = addr_index % THREADS_PER_LANE;
        uint idx = addr_index / THREADS_PER_LANE;
        ulong v = block_th_get(addr, idx);
        v = u64_shuffle(v, thr, thread_id, shuffle_buf);
        ref_index = u64_lo(v);
        ref_lane = u64_hi(v);
    } else {
        ulong v = u64_shuffle(prev->a, 0, thread_id, shuffle_buf);
        ref_index = u64_lo(v);
        ref_lane = u64_hi(v);
    }
    compute_ref_pos(lanes, segment_blocks, pass, lane, slice, offset, &ref_lane, &ref_index);
    argon2_core(memory, mem_curr, prev, tmp, shuffle_buf, lanes, thread_id, pass, ref_index, ref_lane);
}

kernel void argon2_precompute_kernel(
    threadgroup u64_shuffle_buf *shuffle_bufs [[threadgroup(0)]],
    device ref_entry *refs [[buffer(0)]],
    constant precompute_args &args [[buffer(1)]],
    uint3 gid [[thread_position_in_grid]],
    uint3 lid [[thread_position_in_threadgroup]],
    uint3 lsize [[threads_per_threadgroup]]) {
    uint passes = args.passes;
    uint lanes = args.lanes;
    uint segment_blocks = args.segment_blocks;
    uint block_id = gid.x / THREADS_PER_LANE;
    uint warp = lid.x / THREADS_PER_LANE;
    uint thread_id = lid.x % THREADS_PER_LANE;
    threadgroup u64_shuffle_buf *shuffle_buf = &shuffle_bufs[warp];
    uint segment_addr_blocks = (segment_blocks + ARGON2_REFS_PER_BLOCK - 1) / ARGON2_REFS_PER_BLOCK;
    uint segment = block_id / segment_addr_blocks;
    uint block = block_id % segment_addr_blocks;
    uint slice = segment % (ARGON2_SYNC_POINTS / 2);
    uint lane = segment / (ARGON2_SYNC_POINTS / 2);
    uint pass = 0;

    block_th addr;
    block_th tmp;
    uint thread_input;
    switch (thread_id) {
        case 0: thread_input = pass; break;
        case 1: thread_input = lane; break;
        case 2: thread_input = slice; break;
        case 3: thread_input = lanes * segment_blocks * ARGON2_SYNC_POINTS; break;
        case 4: thread_input = passes; break;
        case 5: thread_input = 2; break;
        case 6: thread_input = block + 1; break;
        default: thread_input = 0; break;
    }

    next_addresses(&addr, &tmp, thread_input, thread_id, shuffle_buf);
    refs += segment * segment_blocks;
    for (uint i = 0; i < QWORDS_PER_THREAD; i++) {
        uint pos = i * THREADS_PER_LANE + thread_id;
        uint offset = block * ARGON2_QWORDS_IN_BLOCK + pos;
        if (offset < segment_blocks) {
            ulong v = block_th_get(&addr, i);
            uint ref_index = u64_lo(v);
            uint ref_lane = u64_hi(v);
            compute_ref_pos(lanes, segment_blocks, pass, lane, slice, offset, &ref_lane, &ref_index);
            refs[offset].ref_index = ref_index;
            refs[offset].ref_lane = ref_lane;
        }
    }
}

kernel void argon2_kernel_segment_precompute(
    threadgroup u64_shuffle_buf *shuffle_bufs [[threadgroup(0)]],
    device block_g *memory [[buffer(0)]],
    device const ref_entry *refs [[buffer(1)]],
    constant segment_args &args [[buffer(2)]],
    uint3 gid [[thread_position_in_grid]],
    uint3 lid [[thread_position_in_threadgroup]],
    uint3 lsize [[threads_per_threadgroup]]) {
    uint passes = args.passes;
    uint lanes = args.lanes;
    uint segment_blocks = args.segment_blocks;
    uint pass = args.pass;
    uint slice = args.slice;
    uint job_id = gid.y;
    uint lane = gid.x / THREADS_PER_LANE;
    uint warp = (lid.y * lsize.x + lid.x) / THREADS_PER_LANE;
    uint thread_id = lid.x % THREADS_PER_LANE;
    threadgroup u64_shuffle_buf *shuffle_buf = &shuffle_bufs[warp];
    uint lane_blocks = ARGON2_SYNC_POINTS * segment_blocks;
    memory += (size_t)job_id * lanes * lane_blocks;
    block_th prev;
    block_th tmp;
    device block_g *mem_segment = memory + slice * segment_blocks * lanes + lane;
    device block_g *mem_prev;
    device block_g *mem_curr;
    uint start_offset = 0;
    if (pass == 0) {
        if (slice == 0) {
            mem_prev = mem_segment + 1 * lanes;
            mem_curr = mem_segment + 2 * lanes;
            start_offset = 2;
        } else {
            mem_prev = mem_segment - lanes;
            mem_curr = mem_segment;
        }
    } else {
        mem_prev = mem_segment + (slice == 0 ? lane_blocks * lanes : 0) - lanes;
        mem_curr = mem_segment;
    }
    load_block(&prev, mem_prev, thread_id);
    uint refs_offset = 0;
    if (pass == 0 && slice < ARGON2_SYNC_POINTS / 2) {
        refs_offset = lane * (lane_blocks / 2) + slice * segment_blocks + start_offset;
    }
    for (uint offset = start_offset; offset < segment_blocks; ++offset) {
        argon2_step_precompute(memory, mem_curr, &prev, &tmp, shuffle_buf, refs, &refs_offset, lanes, segment_blocks, thread_id, lane, pass, slice, offset);
        mem_curr += lanes;
    }
}

kernel void argon2_kernel_oneshot_precompute(
    threadgroup u64_shuffle_buf *shuffle_bufs [[threadgroup(0)]],
    device block_g *memory [[buffer(0)]],
    device const ref_entry *refs [[buffer(1)]],
    constant oneshot_args &args [[buffer(2)]],
    uint3 gid [[thread_position_in_grid]],
    uint3 lid [[thread_position_in_threadgroup]],
    uint3 lsize [[threads_per_threadgroup]]) {
    uint passes = args.passes;
    uint lanes = args.lanes;
    uint segment_blocks = args.segment_blocks;
    uint job_id = gid.y;
    uint lane = gid.x / THREADS_PER_LANE;
    uint warp = lid.y * lanes + lid.x / THREADS_PER_LANE;
    uint thread_id = lid.x % THREADS_PER_LANE;
    threadgroup u64_shuffle_buf *shuffle_buf = &shuffle_bufs[warp];
    uint lane_blocks = ARGON2_SYNC_POINTS * segment_blocks;
    memory += (size_t)job_id * lanes * lane_blocks;
    block_th prev;
    block_th tmp;
    device block_g *mem_lane = memory + lane;
    device block_g *mem_prev = mem_lane + 1 * lanes;
    device block_g *mem_curr = mem_lane + 2 * lanes;
    load_block(&prev, mem_prev, thread_id);
    uint refs_offset = lane * (lane_blocks / 2) + 2;
    uint skip = 2;
    for (uint pass = 0; pass < passes; ++pass) {
        for (uint slice = 0; slice < ARGON2_SYNC_POINTS; ++slice) {
            for (uint offset = 0; offset < segment_blocks; ++offset) {
                if (skip > 0) {
                    --skip;
                    continue;
                }
                argon2_step_precompute(memory, mem_curr, &prev, &tmp, shuffle_buf, refs, &refs_offset, lanes, segment_blocks, thread_id, lane, pass, slice, offset);
                mem_curr += lanes;
            }
            threadgroup_barrier(mem_flags::mem_device);
        }
        mem_curr = mem_lane;
    }
}

kernel void argon2_kernel_segment(
    threadgroup u64_shuffle_buf *shuffle_bufs [[threadgroup(0)]],
    device block_g *memory [[buffer(0)]],
    constant segment_args &args [[buffer(1)]],
    uint3 gid [[thread_position_in_grid]],
    uint3 lid [[thread_position_in_threadgroup]],
    uint3 lsize [[threads_per_threadgroup]]) {
    uint passes = args.passes;
    uint lanes = args.lanes;
    uint segment_blocks = args.segment_blocks;
    uint pass = args.pass;
    uint slice = args.slice;
    uint job_id = gid.y;
    uint lane = gid.x / THREADS_PER_LANE;
    uint warp = (lid.y * lsize.x + lid.x) / THREADS_PER_LANE;
    uint thread_id = lid.x % THREADS_PER_LANE;
    threadgroup u64_shuffle_buf *shuffle_buf = &shuffle_bufs[warp];
    uint lane_blocks = ARGON2_SYNC_POINTS * segment_blocks;
    memory += (size_t)job_id * lanes * lane_blocks;
    block_th prev;
    block_th addr;
    block_th tmp;
    uint thread_input;
    switch (thread_id) {
        case 0: thread_input = pass; break;
        case 1: thread_input = lane; break;
        case 2: thread_input = slice; break;
        case 3: thread_input = lanes * lane_blocks; break;
        case 4: thread_input = passes; break;
        case 5: thread_input = 2; break;
        default: thread_input = 0; break;
    }
    if (pass == 0 && slice == 0 && segment_blocks > 2) {
        if (thread_id == 6) {
            ++thread_input;
        }
        next_addresses(&addr, &tmp, thread_input, thread_id, shuffle_buf);
    }
    device block_g *mem_segment = memory + slice * segment_blocks * lanes + lane;
    device block_g *mem_prev;
    device block_g *mem_curr;
    uint start_offset = 0;
    if (pass == 0) {
        if (slice == 0) {
            mem_prev = mem_segment + 1 * lanes;
            mem_curr = mem_segment + 2 * lanes;
            start_offset = 2;
        } else {
            mem_prev = mem_segment - lanes;
            mem_curr = mem_segment;
        }
    } else {
        mem_prev = mem_segment + (slice == 0 ? lane_blocks * lanes : 0) - lanes;
        mem_curr = mem_segment;
    }
    load_block(&prev, mem_prev, thread_id);
    for (uint offset = start_offset; offset < segment_blocks; ++offset) {
        argon2_step(memory, mem_curr, &prev, &tmp, &addr, shuffle_buf, lanes, segment_blocks, thread_id, &thread_input, lane, pass, slice, offset);
        mem_curr += lanes;
    }
}

kernel void argon2_kernel_oneshot(
    threadgroup u64_shuffle_buf *shuffle_bufs [[threadgroup(0)]],
    device block_g *memory [[buffer(0)]],
    constant oneshot_args &args [[buffer(1)]],
    uint3 gid [[thread_position_in_grid]],
    uint3 lid [[thread_position_in_threadgroup]],
    uint3 lsize [[threads_per_threadgroup]]) {
    uint passes = args.passes;
    uint lanes = args.lanes;
    uint segment_blocks = args.segment_blocks;
    uint job_id = gid.y;
    uint lane = gid.x / THREADS_PER_LANE;
    uint warp = lid.y * lanes + lid.x / THREADS_PER_LANE;
    uint thread_id = lid.x % THREADS_PER_LANE;
    threadgroup u64_shuffle_buf *shuffle_buf = &shuffle_bufs[warp];
    uint lane_blocks = ARGON2_SYNC_POINTS * segment_blocks;
    memory += (size_t)job_id * lanes * lane_blocks;
    block_th prev;
    block_th addr;
    block_th tmp;
    uint thread_input;
    switch (thread_id) {
        case 1: thread_input = lane; break;
        case 3: thread_input = lanes * lane_blocks; break;
        case 4: thread_input = passes; break;
        case 5: thread_input = 2; break;
        default: thread_input = 0; break;
    }
    if (segment_blocks > 2) {
        if (thread_id == 6) {
            ++thread_input;
        }
        next_addresses(&addr, &tmp, thread_input, thread_id, shuffle_buf);
    }
    device block_g *mem_lane = memory + lane;
    device block_g *mem_prev = mem_lane + 1 * lanes;
    device block_g *mem_curr = mem_lane + 2 * lanes;
    load_block(&prev, mem_prev, thread_id);
    uint skip = 2;
    for (uint pass = 0; pass < passes; ++pass) {
        for (uint slice = 0; slice < ARGON2_SYNC_POINTS; ++slice) {
            for (uint offset = 0; offset < segment_blocks; ++offset) {
                if (skip > 0) {
                    --skip;
                    continue;
                }
                argon2_step(memory, mem_curr, &prev, &tmp, &addr, shuffle_buf, lanes, segment_blocks, thread_id, &thread_input, lane, pass, slice, offset);
                mem_curr += lanes;
            }
            threadgroup_barrier(mem_flags::mem_device);
            if (thread_id == 2) {
                ++thread_input;
            }
            if (thread_id == 6) {
                thread_input = 0;
            }
        }
        mem_curr = mem_lane;
    }
}
)METAL";

} // namespace app
