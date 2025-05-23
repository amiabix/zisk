use std::sync::Arc;

#[cfg(feature = "debug_mem")]
use num_bigint::ToBigInt;
#[cfg(feature = "debug_mem")]
use std::{
    fs::File,
    io::{BufWriter, Write},
};
use zisk_common::SegmentId;

use crate::{
    MemHelpers, MemInput, MemModule, MEMORY_MAX_DIFF, MEM_BYTES_BITS, STEP_MEMORY_LIMIT_TO_VERIFY,
    STEP_MEMORY_MAX_DIFF,
};
use p3_field::PrimeField64;
use pil_std_lib::Std;
use proofman_common::{AirInstance, FromTrace};

use zisk_core::{RAM_ADDR, RAM_SIZE};
use zisk_pil::{MemAirValues, MemTrace};

pub const RAM_W_ADDR_INIT: u32 = RAM_ADDR as u32 >> MEM_BYTES_BITS;
pub const RAM_W_ADDR_END: u32 = (RAM_ADDR + RAM_SIZE - 1) as u32 >> MEM_BYTES_BITS;

const _: () = {
    assert!(
        (RAM_ADDR + RAM_SIZE - 1) <= 0xFFFF_FFFF,
        "RAM memory exceeds the 32-bit addressable range"
    );
};

pub struct MemSM<F: PrimeField64> {
    /// PIL2 standard library
    std: Arc<Std<F>>,
}
#[derive(Debug, Default)]
pub struct MemPreviousSegment {
    pub addr: u32,
    pub step: u64,
    pub value: u64,
    pub extra_zero_step: bool,
}

#[allow(unused, unused_variables)]
impl<F: PrimeField64> MemSM<F> {
    pub fn new(std: Arc<Std<F>>) -> Arc<Self> {
        Arc::new(Self { std: std.clone() })
    }

    pub fn get_to_addr() -> u32 {
        (RAM_ADDR + RAM_SIZE - 1) as u32
    }
    #[cfg(feature = "debug_mem")]
    pub fn save_to_file(&self, trace: &MemTrace<F>, file_name: &str) {
        println!("[MemDebug] writing information {} .....", file_name);
        let file = File::create(file_name).unwrap();
        let mut writer = BufWriter::new(file);
        let num_rows = MemTrace::<usize>::NUM_ROWS;

        for i in 0..num_rows {
            let addr = trace[i].addr.as_canonical_biguint().to_bigint().unwrap() * 8;
            let step = trace[i].step.as_canonical_biguint().to_bigint().unwrap();
            writeln!(
                writer,
                "{:#010X} {} {} {:?}",
                addr, trace[i].step, trace[i].wr, trace[i].value
            )
            .unwrap();
        }
        println!("[MemDebug] done");
    }
}

impl<F: PrimeField64> MemModule<F> for MemSM<F> {
    fn get_addr_range(&self) -> (u32, u32) {
        (RAM_W_ADDR_INIT, RAM_W_ADDR_END)
    }
    /// Finalizes the witness accumulation process and triggers the proof generation.
    ///
    /// This method is invoked by the executor when no further witness data remains to be added.
    ///
    /// # Parameters
    ///
    /// - `mem_inputs`: A slice of all `MemoryInput` inputs
    fn compute_witness(
        &self,
        mem_ops: &[MemInput],
        segment_id: SegmentId,
        is_last_segment: bool,
        previous_segment: &MemPreviousSegment,
    ) -> AirInstance<F> {
        let mut trace = MemTrace::<F>::new();

        // println!(
        //     "[MemSM] segment_id:{} mem_ops:{} rows:{}  [0]{:?} previous_segment:{:?}",
        //     segment_id,
        //     mem_ops.len(),
        //     trace.num_rows,
        //     mem_ops[0],
        //     previous_segment
        // );

        let std = self.std.clone();

        let range_id = std.get_range(1, MEMORY_MAX_DIFF as i64, None);
        let mut range_check_data: Vec<u16> = vec![0; MEMORY_MAX_DIFF as usize];
        let f_range_check_max_value = 0xFFFF + 1;

        // use special counter for internal reads
        let mut range_check_data_max = 0u64;
        let mut range_check_data_min = 0u64;

        let distance_base = previous_segment.addr - RAM_W_ADDR_INIT;
        let mut last_addr = previous_segment.addr;
        let mut last_step = previous_segment.step;
        let mut last_value = previous_segment.value;
        let mut force_zero_step = previous_segment.extra_zero_step;
        // let mut total_full_rows = 0u64;
        // let mut total_zero_rows = 0u64;

        let mut i = 0;
        let mut increment;

        // f_max_increment it's plus 1 because on read operations we increment the step
        // difference in one, to allow read the same address with "same" step
        let f_max_increment = F::from_u64(STEP_MEMORY_MAX_DIFF + 1);

        #[cfg(feature = "debug_mem")]
        let mut _mem_op_done = 0;
        // let mut debug_last_addr = 0;
        // let mut debug_last_chunk = ChunkId(0);
        // let mut debug_last_i = 0;

        for mem_op in mem_ops {
            let mut step = mem_op.step;

            if i >= trace.num_rows {
                break;
            }

            // set the common values of trace between internal reads and regular memory operation
            trace[i].addr = F::from_u32(mem_op.addr);
            let addr_changes = last_addr != mem_op.addr;
            trace[i].addr_changes = if addr_changes { F::ONE } else { F::ZERO };

            if addr_changes {
                increment = (mem_op.addr - last_addr) as u64;
            } else {
                if step < last_step {
                    panic!(
                        "MemSM: step < last_step {} < {} addr_changes:{} mem_op.addr:0x{:X} last_addr:0x{:X} mem_op.step:{} last_step:{} row:{} previous:{:?}",
                        step, last_step, addr_changes as u8, mem_op.addr * 8, last_addr * 8, mem_op.step, last_step, i, previous_segment
                    );
                }
                increment = step - last_step;
                if increment as usize >= STEP_MEMORY_LIMIT_TO_VERIFY || force_zero_step {
                    // could be that no has internal reads, but need to check.
                    if let Some((mut full_rows, mut zero_row)) =
                        MemHelpers::forced_get_intermediate_rows(last_step, step, force_zero_step)
                    {
                        let internal_reads = full_rows + zero_row;
                        let incomplete = (i + internal_reads as usize) >= trace.num_rows;

                        // if segment_id >= SegmentId(52) && segment_id <= SegmentId(55) {
                        //     println!(
                        //         "INTERNAL_READS[{},{}] {} 0x{:X},{} [{},{}] [{},{}]",
                        //         segment_id,
                        //         i,
                        //         internal_reads,
                        //         mem_op.addr * 8,
                        //         step,
                        //         full_rows,
                        //         zero_row,
                        //         last_step,
                        //         step
                        //     );
                        // }

                        // check if has enough rows to complete the internal reads + regular memory
                        if (i + internal_reads as usize) > trace.num_rows {
                            full_rows = (trace.num_rows - i) as u64;
                            zero_row = 0;
                        }

                        // total_zero_rows += zero_row;
                        // total_full_rows += full_rows;
                        // without address changes, the internal reads before write must use the last
                        // value, in the case of reads value and the last value are the same
                        let (low_val, high_val) = (last_value as u32, (last_value >> 32) as u32);
                        trace[i].value = [F::from_u32(low_val), F::from_u32(high_val)];

                        // it's intenal
                        trace[i].sel = F::ZERO;

                        // in internal reads the increment is always the max increment
                        if full_rows > 0 {
                            trace[i].increment = f_max_increment;
                            // set step as max increment from last_step
                            step = last_step + STEP_MEMORY_MAX_DIFF;
                        } else {
                            // in case of zero_row = 1, full_rows = 0
                            // increment is 0 + (1 - wr) = 1
                            trace[i].increment = F::ONE;
                            step = last_step;
                            range_check_data_min += 1;
                        }

                        // internal reads always must be read
                        trace[i].wr = F::ZERO;

                        // setting step on trace
                        trace[i].step = F::from_u64(step);

                        // update last_step and increment step
                        last_step = step;

                        i += 1;

                        // the trace values of the rest of internal reads are equal to previous, only
                        // change the value of step
                        for _j in 1..full_rows {
                            trace[i] = trace[i - 1];
                            step += STEP_MEMORY_MAX_DIFF;
                            trace[i].step = F::from_u64(step);
                            last_step = step;
                            i += 1;
                        }
                        if zero_row > 0 && full_rows > 0 {
                            // in case of zero_row = 1, full_rows = 0 really no need to add this
                            // extra step, because it was added previously on first row.

                            // row with zero increment, step was the same, necessary extra row added
                            // because counters don't has information about previous or last step for
                            // each address
                            trace[i] = trace[i - 1];
                            // increment zero is allowed, because when operation is a read, increase
                            // in one the increment. With this feature a mem position can be read multiple
                            // times in same mem-step (timestamp)
                            // increment is 0 + (1 - wr) = 1
                            trace[i].increment = F::ONE;
                            range_check_data_min += 1;
                            i += 1;
                        }

                        range_check_data_max += full_rows;

                        // control the edge case when there aren't enough rows to complete the internal
                        // reads or regular memory operation
                        if incomplete {
                            last_addr = mem_op.addr;
                            break;
                        }
                        step = mem_op.step;
                        increment = step - last_step;

                        // copy last trace for the regular memory operation (addr, addr_changes)
                        trace[i] = trace[i - 1];
                    }
                }
                force_zero_step = false;
            }

            if i >= trace.num_rows {
                break;
            }
            // set specific values of trace for regular memory operation
            let (low_val, high_val) = (mem_op.value as u32, (mem_op.value >> 32) as u32);
            trace[i].value = [F::from_u32(low_val), F::from_u32(high_val)];

            trace[i].step = F::from_u64(step);
            trace[i].sel = F::ONE;

            if !addr_changes && !mem_op.is_write {
                // in case of read operations of same address, add one to allow many reads
                // over same address and step
                increment += 1;
            }
            trace[i].increment = F::from_u64(increment);
            trace[i].wr = F::from_bool(mem_op.is_write);
            // println!("TRACE[{}] = [0x{:X},{}] {}", i, mem_op.addr * 8, mem_op.step, mem_op.value,);

            #[cfg(feature = "debug_mem")]
            {
                _mem_op_done += 1;
            }

            // Store the value of incremenet so it can be range checked
            let range_index = increment as usize - 1;
            if range_index < MEMORY_MAX_DIFF as usize {
                if range_check_data[range_index] == 0xFFFF {
                    range_check_data[range_index] = 0;
                    std.range_check(increment as i64, f_range_check_max_value, range_id);
                } else {
                    range_check_data[range_index] += 1;
                }
            } else {
                panic!("MemSM: increment's out of range: {} i:{} addr_changes:{} mem_op.addr:0x{:X} last_addr:0x{:X} mem_op.step:{} last_step:{}",
                    increment, i, addr_changes as u8, mem_op.addr, last_addr, mem_op.step, last_step);
            }

            last_addr = mem_op.addr;
            last_step = step;
            last_value = mem_op.value;
            i += 1;
        }
        let count = i;

        // STEP3. Add dummy rows to the output vector to fill the remaining rows
        // PADDING: At end of memory fill with same addr, incrementing step, same value, sel = 0, rd
        // = 1, wr = 0
        let last_row_idx = count - 1;
        let addr = trace[last_row_idx].addr;
        let value = trace[last_row_idx].value;

        // Two situations with padding, at end of all segments, where there aren't more operations,
        // in this case we increment step one-by-one. The second situation is in the middle of
        // padding between step with distance too large, in this case we increment with maximum
        // allowed distance.
        let padding_size = trace.num_rows - count;
        let padding_step = if is_last_segment { 1 } else { STEP_MEMORY_MAX_DIFF };

        // if segment_id >= SegmentId(52) && segment_id <= SegmentId(55) {
        //     println!(
        //         "INTERNAL_READS[{},{}] {} 0x{:X},{} .... END",
        //         segment_id,
        //         i,
        //         padding_size,
        //         addr.as_canonical_u64() * 8,
        //         last_step,
        //     );
        // }

        let padding_increment = F::from_u64(padding_step + 1);
        for i in count..trace.num_rows {
            last_step += padding_step;
            trace[i].addr = addr;
            trace[i].step = F::from_u64(last_step);
            trace[i].sel = F::ZERO;
            trace[i].wr = F::ZERO;

            trace[i].value = value;

            trace[i].addr_changes = F::ZERO;
            trace[i].increment = padding_increment;
        }
        if padding_size > 0 {
            // Store the padding range checks
            self.std.range_check((padding_step + 1) as i64, padding_size as u64, range_id);
        }

        // if segment_id >= SegmentId(52) && segment_id <= SegmentId(54) {
        //     for irow in (0..200).chain(4194100..4194304) {
        //         println!(
        //             "TRACE ROWS[{},{}] 0x{:X},{} V:{},{} {}",
        //             segment_id,
        //             irow,
        //             trace[irow].addr.as_canonical_u64() * 8,
        //             trace[irow].step,
        //             trace[irow].value[0].as_canonical_u64(),
        //             trace[irow].value[1].as_canonical_u64(),
        //             if trace[irow].wr == F::ZERO {
        //                 if trace[irow].sel == F::ZERO {
        //                     "INT"
        //                 } else {
        //                     "RD"
        //                 }
        //             } else {
        //                 "WR"
        //             }
        //         );
        //     }
        // }

        // no add extra +1 because index = value - 1
        // RAM_W_ADDR_END - last_addr + 1 - 1 = RAM_W_ADDR_END - last_addr
        let distance_end = RAM_W_ADDR_END - last_addr;

        for (value, &multiplicity) in range_check_data.iter().enumerate() {
            if multiplicity == 0 {
                continue;
            }
            self.std.range_check((value + 1) as i64, multiplicity as u64, range_id);
        }
        // Add one in range_check_data_max because it's used by intermediate reads, and reads
        // add one to distance to allow same step on read operations.

        if range_check_data_max > 0 {
            self.std.range_check((STEP_MEMORY_MAX_DIFF + 1) as i64, range_check_data_max, range_id);
        }
        if range_check_data_min > 0 {
            self.std.range_check(1, range_check_data_min, range_id);
        }

        let mut air_values = MemAirValues::<F>::new();
        air_values.segment_id = F::from_usize(segment_id.into());
        air_values.is_first_segment = F::from_bool(segment_id == 0);
        air_values.is_last_segment = F::from_bool(is_last_segment);
        air_values.previous_segment_step = F::from_u64(previous_segment.step);
        air_values.previous_segment_addr = F::from_u32(previous_segment.addr);
        air_values.segment_last_addr = F::from_u32(last_addr);
        air_values.segment_last_step = F::from_u64(last_step);

        air_values.previous_segment_value[0] = F::from_u32(previous_segment.value as u32);
        air_values.previous_segment_value[1] = F::from_u32((previous_segment.value >> 32) as u32);

        air_values.segment_last_value[0] = F::from_u32(last_value as u32);
        air_values.segment_last_value[1] = F::from_u32((last_value >> 32) as u32);

        let distance_base = [distance_base as u16, (distance_base >> 16) as u16];
        let distance_end = [distance_end as u16, (distance_end >> 16) as u16];

        air_values.distance_base[0] = F::from_u16(distance_base[0]);
        air_values.distance_base[1] = F::from_u16(distance_base[1]);

        air_values.distance_end[0] = F::from_u16(distance_end[0]);
        air_values.distance_end[1] = F::from_u16(distance_end[1]);

        // println!("AIR_VALUES[{}]: {:?}", segment_id, air_values);

        let range_16bits_id = std.get_range(0, 0xFFFF, None);

        self.std.range_check(distance_base[0] as i64, 1, range_16bits_id);
        self.std.range_check(distance_base[1] as i64, 1, range_16bits_id);
        self.std.range_check(distance_end[0] as i64, 1, range_16bits_id);
        self.std.range_check(distance_end[1] as i64, 1, range_16bits_id);

        #[cfg(feature = "debug_mem")]
        {
            self.save_to_file(&trace, &format!("/tmp/mem_trace_{}.txt", segment_id));
            println!(
                "[Mem:{}] mem_ops:{}/{} padding:{}",
                segment_id,
                _mem_op_done,
                mem_ops.len(),
                padding_size
            );
        }
        // if total_zero_rows > 0 {
        //     println!(
        //         "[Mem:{}] full_rows: {} zero_rows: {}",
        //         segment_id, total_full_rows, total_zero_rows
        //     );
        // }
        AirInstance::new_from_trace(FromTrace::new(&mut trace).with_air_values(&mut air_values))
    }
}
