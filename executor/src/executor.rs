//! The `ZiskExecutor` module serves as the core orchestrator for executing the ZisK ROM program
//! and generating witness computations. It manages the execution of the state machines, from initial
//! planning to witness computation, ensuring efficient parallel processing and resource
//! utilization.
//!
//! This module handles both main and secondary state machines, integrating complex tasks such as
//! planning, configuration, and witness generation into a streamlined process.
//!
//! ## Executor Workflow
//! The execution is divided into distinct, sequential phases:
//!
//! 1. **Minimal Traces**: Rapidly process the ROM to collect minimal traces with minimal overhead.
//! 2. **Counting**: Creates the metrics required for the secondary state machine instances.
//! 3. **Planning**: Strategically plan the execution of instances to optimize resource usage.
//! 4. **Instance Creation**: Creates the AIR instances for the main and secondary state machines.
//! 5. **Witness Computation**: Compute the witnesses for all AIR instances, leveraging parallelism
//!    for efficiency.
//!
//! By structuring these phases, the `ZiskExecutor` ensures high-performance execution while
//! maintaining clarity and modularity in the computation process.

use asm_runner::{AsmRunnerMT, MinimalTraces, Task, TaskFactory};
use p3_field::PrimeField64;
use pil_std_lib::Std;
use proofman_common::{ProofCtx, SetupCtx};
use proofman_util::{timer_start_info, timer_stop_and_log_info};
use rom_setup::gen_elf_hash;
use witness::WitnessComponent;

use rayon::prelude::*;

use crate::{DataBusCollectorCollection, DummyCounter};
use data_bus::DataBusTrait;
use sm_main::{MainInstance, MainPlanner, MainSM};
use zisk_common::{
    BusDevice, BusDeviceMetrics, CheckPoint, Instance, InstanceCtx, InstanceType, Plan,
};
use zisk_common::{ChunkId, PayloadType};
use zisk_pil::{RomRomTrace, ZiskPublicValues, MAIN_AIR_IDS};

use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};
use zisk_common::EmuTrace;
use zisk_core::ZiskRom;
use ziskemu::{EmuOptions, ZiskEmulator};

use crate::SMBundle;

type DeviceMetricsByChunk = (ChunkId, Box<dyn BusDeviceMetrics>); // (chunk_id, metrics)
type DeviceMetricsList = Vec<DeviceMetricsByChunk>;
pub type NestedDeviceMetricsList = Vec<DeviceMetricsList>;

#[derive(Debug, Default, Clone)]
pub struct ZiskExecutionResult {
    pub executed_steps: u64,
}

#[allow(dead_code)]
enum MinimalTraceExecutionMode {
    Emulator,
    Asm,
    AsmWithCounter,
}

/// The `ZiskExecutor` struct orchestrates the execution of the ZisK ROM program, managing state
/// machines, planning, and witness computation.
pub struct ZiskExecutor<F: PrimeField64, BD: SMBundle<F>> {
    /// ZisK ROM, a binary file containing the ZisK program to be executed.
    pub zisk_rom: Arc<ZiskRom>,

    /// Path to the input data file.
    pub input_data_path: Option<PathBuf>,

    pub rom_path: PathBuf,

    pub asm_runner_path: Option<PathBuf>,
    pub asm_rom_path: Option<PathBuf>,

    /// Planning information for main state machines.
    pub min_traces: RwLock<MinimalTraces>,
    pub main_planning: RwLock<Vec<Plan>>,
    pub secn_planning: RwLock<Vec<Vec<Plan>>>,

    pub main_instances: RwLock<HashMap<usize, MainInstance>>,
    pub secn_instances: RwLock<HashMap<usize, Box<dyn Instance<F>>>>,
    std: Arc<Std<F>>,

    execution_result: Mutex<ZiskExecutionResult>,

    main_count: Mutex<Option<DeviceMetricsList>>,
    secn_count: Mutex<Option<NestedDeviceMetricsList>>,
    sm_bundle: BD,
}

impl<F: PrimeField64, BD: SMBundle<F>> ZiskExecutor<F, BD> {
    /// The number of threads to use for parallel processing when computing minimal traces.
    const NUM_THREADS: usize = 16;

    /// The size in rows of the minimal traces
    const MIN_TRACE_SIZE: u64 = 1 << 18;

    const MAX_NUM_STEPS: u64 = 1 << 32;

    /// Creates a new instance of the `ZiskExecutor`.
    ///
    /// # Arguments
    /// * `input_data_path` - Path to the input data file.
    /// * `zisk_rom` - An `Arc`-wrapped ZisK ROM instance.
    pub fn new(
        rom_path: PathBuf,
        asm_path: Option<PathBuf>,
        asm_rom_path: Option<PathBuf>,
        input_data_path: Option<PathBuf>,
        zisk_rom: Arc<ZiskRom>,
        std: Arc<Std<F>>,
        sm_bundle: BD,
    ) -> Self {
        Self {
            input_data_path,
            rom_path,
            asm_runner_path: asm_path,
            asm_rom_path,
            zisk_rom,
            min_traces: RwLock::new(MinimalTraces::None),
            main_planning: RwLock::new(Vec::new()),
            secn_planning: RwLock::new(Vec::new()),
            main_instances: RwLock::new(HashMap::new()),
            secn_instances: RwLock::new(HashMap::new()),
            std,
            execution_result: Mutex::new(ZiskExecutionResult::default()),
            main_count: Mutex::new(None),
            secn_count: Mutex::new(None),
            sm_bundle,
        }
    }

    pub fn get_execution_result(&self) -> ZiskExecutionResult {
        self.execution_result.lock().unwrap().clone()
    }

    /// Computes minimal traces by processing the ZisK ROM with given public inputs.
    ///
    /// # Arguments
    /// * `input_data` - Input data for the ROM execution.
    /// * `num_threads` - Number of threads to use for parallel execution.
    ///
    /// # Returns
    /// A vector of `EmuTrace` instances representing minimal traces.
    fn compute_minimal_traces(&self, mode: MinimalTraceExecutionMode) -> MinimalTraces {
        let min_traces = match mode {
            MinimalTraceExecutionMode::Emulator => self.run_emulator(Self::NUM_THREADS),
            MinimalTraceExecutionMode::Asm => self.run_assembly(),
            MinimalTraceExecutionMode::AsmWithCounter => self.run_and_count_assembly(),
        };

        // Store execute steps
        let steps = match &min_traces {
            MinimalTraces::None => {
                panic!("Error during minimal traces computation");
            }
            MinimalTraces::EmuTrace(min_traces) => {
                min_traces.iter().map(|trace| trace.steps).sum::<u64>()
            }
            MinimalTraces::AsmEmuTrace(asm_min_traces) => {
                asm_min_traces.vec_chunks.iter().map(|trace| trace.steps).sum::<u64>()
            }
        };

        self.execution_result.lock().unwrap().executed_steps = steps;

        min_traces
    }

    fn run_assembly(&self) -> MinimalTraces {
        MinimalTraces::AsmEmuTrace(AsmRunnerMT::run(
            self.asm_runner_path.as_ref().unwrap(),
            self.input_data_path.as_ref().unwrap(),
            Self::MAX_NUM_STEPS,
            Self::MIN_TRACE_SIZE,
            asm_runner::AsmRunnerOptions::default(),
        ))
    }

    fn run_and_count_assembly(&self) -> MinimalTraces {
        struct CounterTask<F, DB>
        where
            DB: DataBusTrait<PayloadType, Box<dyn BusDeviceMetrics>>,
        {
            chunk_id: ChunkId,
            emu_trace: EmuTrace,
            data_bus: Mutex<Option<DB>>,
            zisk_rom: Arc<ZiskRom>,
            _phantom: std::marker::PhantomData<F>,
        }

        impl<F, DB> Task for CounterTask<F, DB>
        where
            F: PrimeField64,
            DB: DataBusTrait<PayloadType, Box<dyn BusDeviceMetrics>> + Send + Sync + 'static,
        {
            type Output = (ChunkId, DB);

            fn execute(&self) -> Self::Output {
                let mut data_bus = self.data_bus.lock().unwrap();
                let mut data_bus = std::mem::take(&mut *data_bus).unwrap();

                ZiskEmulator::process_emu_trace::<F, _, _>(
                    &self.zisk_rom,
                    &self.emu_trace,
                    &mut data_bus,
                );

                data_bus.on_close();

                (self.chunk_id, data_bus)
            }
        }

        impl<F, DB> Drop for CounterTask<F, DB>
        where
            DB: DataBusTrait<PayloadType, Box<dyn BusDeviceMetrics>>,
        {
            fn drop(&mut self) {
                std::mem::forget(std::mem::take(&mut self.emu_trace.mem_reads));
            }
        }

        let task_factory: TaskFactory<_> = Box::new(|chunk_id: ChunkId, emu_trace: EmuTrace| {
            let data_bus = self.sm_bundle.build_data_bus_counters();
            CounterTask {
                chunk_id,
                emu_trace,
                data_bus: Mutex::new(Some(data_bus)),
                zisk_rom: self.zisk_rom.clone(),
                _phantom: std::marker::PhantomData::<F>,
            }
        });

        let (asm_runner_mt, mut data_buses) = AsmRunnerMT::run_and_count(
            self.asm_runner_path.as_ref().unwrap(),
            self.input_data_path.as_ref().unwrap(),
            Self::MAX_NUM_STEPS,
            Self::MIN_TRACE_SIZE,
            asm_runner::AsmRunnerOptions::default(),
            task_factory,
        );

        data_buses.sort_by_key(|(chunk_id, _)| chunk_id.0);

        let mut main_count = Vec::with_capacity(data_buses.len());
        let mut secn_count = Vec::with_capacity(data_buses.len());

        let main_idx = self.sm_bundle.main_counter_idx();
        for (chunk_id, data_bus) in data_buses {
            let databus_counters = data_bus.into_devices(false);

            let mut secondary = Vec::new();

            for (idx, counter) in databus_counters.into_iter().enumerate() {
                match main_idx {
                    None => secondary.push((chunk_id, counter)),
                    Some(i) if idx == i => {
                        main_count.push((chunk_id, counter.unwrap_or(Box::new(DummyCounter {}))))
                    }
                    Some(_) => secondary.push((chunk_id, counter)),
                }
            }

            secn_count.push(secondary);
        }

        // Group counters by chunk_id and counter type
        let mut secn_vec_counters =
            (0..secn_count[0].len()).map(|_| Vec::new()).collect::<Vec<_>>();

        secn_count.into_iter().for_each(|counter_slice| {
            counter_slice.into_iter().enumerate().for_each(|(i, (chunk_id, counter))| {
                secn_vec_counters[i].push((chunk_id, counter.unwrap_or(Box::new(DummyCounter {}))));
            });
        });

        self.main_count.lock().unwrap().replace(main_count);
        self.secn_count.lock().unwrap().replace(secn_vec_counters);

        MinimalTraces::AsmEmuTrace(asm_runner_mt)
    }

    fn run_emulator(&self, num_threads: usize) -> MinimalTraces {
        assert!(Self::MIN_TRACE_SIZE.is_power_of_two());

        // Call emulate with these options
        let input_data = if self.input_data_path.is_some() {
            // Read inputs data from the provided inputs path
            let path = PathBuf::from(self.input_data_path.as_ref().unwrap().display().to_string());
            fs::read(path).expect("Could not read inputs file")
        } else {
            Vec::new()
        };

        // Settings for the emulator
        let emu_options = EmuOptions {
            trace_steps: Some(Self::MIN_TRACE_SIZE),
            max_steps: Self::MAX_NUM_STEPS,
            ..EmuOptions::default()
        };

        let min_traces = ZiskEmulator::compute_minimal_traces(
            &self.zisk_rom,
            &input_data,
            &emu_options,
            num_threads,
        )
        .expect("Error during emulator execution");

        MinimalTraces::EmuTrace(min_traces)
    }

    /// Adds main state machine instances to the proof context and assigns global IDs.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    /// * `main_planning` - Planning information for main state machines.
    fn assign_main_instances(&self, pctx: &ProofCtx<F>, main_planning: &mut [Plan]) {
        for plan in main_planning.iter_mut() {
            plan.set_global_id(pctx.add_instance(plan.airgroup_id, plan.air_id));
        }
    }

    /// Creates main state machine instance based on a main planning.
    ///
    /// # Arguments
    /// * `global_id` - Global ID of the main instance to be created.
    ///
    /// # Returns
    /// A main instance for the provided global ID.
    fn create_main_instance(&self, global_id: usize) -> MainInstance {
        let mut main_planning_guard = self.main_planning.write().unwrap();

        let plan_idx = main_planning_guard
            .iter()
            .position(|x| x.global_id.unwrap() == global_id)
            .expect("Main instance not found");

        let plan = main_planning_guard.remove(plan_idx);

        let global_id = plan.global_id.unwrap();
        let is_last_segment = *plan
            .meta
            .as_ref()
            .and_then(|m| m.downcast_ref::<bool>())
            .unwrap_or_else(|| panic!("create_main_instance: Invalid metadata format"));

        MainInstance::new(InstanceCtx::new(global_id, plan), is_last_segment)
    }

    /// Counts metrics for secondary state machines based on minimal traces.
    ///
    /// # Arguments
    /// * `min_traces` - Minimal traces obtained from the ROM execution.
    ///
    /// # Returns
    /// A tuple containing two vectors:
    /// * A vector of main state machine metrics grouped by chunk ID.
    /// * A vector of secondary state machine metrics grouped by chunk ID. The vector is nested,
    ///   with the outer vector representing the secondary state machines and the inner vector
    ///   containing the metrics for each chunk.
    fn count(&self, min_traces: &MinimalTraces) -> (DeviceMetricsList, NestedDeviceMetricsList) {
        let min_traces = match min_traces {
            MinimalTraces::EmuTrace(min_traces) => min_traces,
            MinimalTraces::AsmEmuTrace(asm_min_traces) => &asm_min_traces.vec_chunks,
            _ => unreachable!(),
        };

        let (main_metrics_slices, secn_metrics_slices): (Vec<_>, Vec<_>) = min_traces
            .par_iter()
            .map(|minimal_trace| {
                let mut data_bus = self.sm_bundle.build_data_bus_counters();

                ZiskEmulator::process_emu_trace::<F, _, _>(
                    &self.zisk_rom,
                    minimal_trace,
                    &mut data_bus,
                );

                let (mut main_count, mut secn_count) = (Vec::new(), Vec::new());

                let databus_counters = data_bus.into_devices(true);
                let main_idx = self.sm_bundle.main_counter_idx();
                for (idx, counter) in databus_counters.into_iter().enumerate() {
                    match main_idx {
                        None => secn_count.push(counter),
                        Some(i) if idx == i => main_count.push(counter),
                        Some(_) => secn_count.push(counter),
                    }
                }
                (main_count, secn_count)
            })
            .unzip();

        // Group counters by chunk_id and counter type
        let mut secn_vec_counters =
            (0..secn_metrics_slices[0].len()).map(|_| Vec::new()).collect::<Vec<_>>();

        secn_metrics_slices.into_iter().enumerate().for_each(|(chunk_id, counter_slice)| {
            counter_slice.into_iter().enumerate().for_each(|(i, counter)| {
                secn_vec_counters[i]
                    .push((ChunkId(chunk_id), counter.unwrap_or(Box::new(DummyCounter {}))));
            });
        });

        let main_vec_counters: Vec<_> = main_metrics_slices
            .into_iter()
            .enumerate()
            .flat_map(|(chunk_id, counters)| {
                counters.into_iter().map(move |counter| {
                    (ChunkId(chunk_id), counter.unwrap_or(Box::new(DummyCounter {})))
                })
            })
            .collect();

        (main_vec_counters, secn_vec_counters)
    }

    /// Adds secondary state machine instances to the proof context and assigns global IDs.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    /// * `secn_planning` - Planning information for secondary state machines.
    fn assign_secn_instances(&self, pctx: &ProofCtx<F>, secn_planning: &mut [Vec<Plan>]) {
        for plans_by_sm in secn_planning.iter_mut() {
            for plan in plans_by_sm.iter_mut() {
                let global_id = match plan.instance_type {
                    InstanceType::Instance => pctx.add_instance(plan.airgroup_id, plan.air_id),
                    InstanceType::Table => pctx.add_instance_all(plan.airgroup_id, plan.air_id),
                };
                plan.set_global_id(global_id);
            }
        }
    }

    /// Creates a secondary state machine instance based on the provided global ID.
    ///
    /// # Arguments
    /// * `global_id` - Global ID of the secondary state machine instance.
    ///
    /// # Returns
    /// A secondary state machine instance for the provided global ID.
    fn create_secn_instance(&self, global_id: usize) -> Box<dyn Instance<F>> {
        let mut secn_planning_guard = self.secn_planning.write().unwrap();

        let plan_idx = secn_planning_guard.iter().enumerate().find_map(|(outer_idx, plans)| {
            plans
                .iter()
                .position(|plan| plan.global_id.unwrap() == global_id)
                .map(|inner_idx| (outer_idx, inner_idx))
        });
        if plan_idx.is_none() {
            panic!("Secondary instance not found");
        }

        let plan_idx = plan_idx.unwrap();
        let plan = secn_planning_guard[plan_idx.0].remove(plan_idx.1);

        let global_id = plan.global_id.unwrap();

        let ictx = InstanceCtx::new(global_id, plan);
        self.sm_bundle.build_instance(plan_idx.0, ictx)
    }

    /// Expands and computes witnesses for a main instance.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    /// * `main_instance` - Main instance to compute witness for
    fn witness_main_instance(&self, pctx: &ProofCtx<F>, main_instance: &mut MainInstance) {
        let min_traces_guard = self.min_traces.read().unwrap();
        let min_traces = &*min_traces_guard;

        let min_traces = match min_traces {
            MinimalTraces::EmuTrace(min_traces) => min_traces,
            MinimalTraces::AsmEmuTrace(asm_min_traces) => &asm_min_traces.vec_chunks,
            _ => unreachable!(),
        };

        let air_instance = MainSM::compute_witness(
            &self.zisk_rom,
            min_traces,
            Self::MIN_TRACE_SIZE,
            main_instance,
            self.std.clone(),
        );

        pctx.add_air_instance(air_instance, main_instance.ictx.global_id);
    }

    /// Expands and computes witness for a secondary state machines instance.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    /// * `sctx` - Setup context.
    /// * `global_id` - Global ID of the secondary state machine instance.
    /// * `secn_instance` - Secondary state machine instance to compute witness for
    fn witness_secn_instance(
        &self,
        pctx: &ProofCtx<F>,
        sctx: &SetupCtx<F>,
        global_id: usize,
        secn_instance: &mut Box<dyn Instance<F>>,
    ) {
        assert_eq!(secn_instance.instance_type(), InstanceType::Instance, "Instance is a table");

        let min_traces = self.min_traces.read().unwrap();

        let min_traces = match &*min_traces {
            MinimalTraces::EmuTrace(min_traces) => min_traces,
            MinimalTraces::AsmEmuTrace(asm_min_traces) => &asm_min_traces.vec_chunks,
            _ => unreachable!(),
        };

        // Group the instances by the chunk they need to process
        let chunks_to_execute = self.chunks_to_execute(min_traces, secn_instance);

        // Create data buses for each chunk
        let mut data_buses =
            self.sm_bundle.build_data_bus_collectors(secn_instance, chunks_to_execute);

        // Execute collect process for each chunk
        data_buses.par_iter_mut().enumerate().for_each(|(chunk_id, data_bus)| {
            if let Some(data_bus) = data_bus {
                ZiskEmulator::process_emu_traces::<F, _, _>(
                    &self.zisk_rom,
                    min_traces,
                    chunk_id,
                    data_bus,
                );
            }
        });

        // Close the data buses and get for each instance its collectors
        let collectors_by_instance = self.close_data_bus_collectors(data_buses);

        if let Some(air_instance) =
            secn_instance.compute_witness(pctx, sctx, collectors_by_instance)
        {
            pctx.add_air_instance(air_instance, global_id);
        }
    }

    /// Computes and generates witness for secondary state machine instance of type `Table`.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    /// * `sctx` - Setup context.
    /// * `global_id` - Global ID of the secondary state machine instance.
    /// * `table_instance` - Secondary state machine table instance to compute witness for
    fn witness_table(
        &self,
        pctx: &ProofCtx<F>,
        sctx: &SetupCtx<F>,
        global_id: usize,
        table_instance: &mut Box<dyn Instance<F>>,
    ) {
        assert_eq!(table_instance.instance_type(), InstanceType::Table, "Instance is not a table");

        if let Some(air_instance) = table_instance.compute_witness(pctx, sctx, vec![]) {
            if pctx.dctx_is_my_instance(global_id) {
                pctx.add_air_instance(air_instance, global_id);
            }
        }
    }

    /// Computes all the chunks to be executed to generate the witness given an instance.
    ///
    /// # Arguments
    /// * `min_traces` - Minimal traces
    /// * `secn_instance` - Secondary state machine instance to group.
    ///
    /// # Returns
    /// A vector of booleans indicating which chunks to execute.
    fn chunks_to_execute(
        &self,
        min_traces: &[EmuTrace],
        secn_instance: &mut Box<dyn Instance<F>>,
    ) -> Vec<bool> {
        let mut chunks_to_execute = vec![false; min_traces.len()];

        match secn_instance.check_point() {
            CheckPoint::None => {}
            CheckPoint::Single(chunk_id) => {
                chunks_to_execute[chunk_id.as_usize()] = true;
            }
            CheckPoint::Multiple(chunk_ids) => {
                chunk_ids.iter().for_each(|&chunk_id| {
                    chunks_to_execute[chunk_id.as_usize()] = true;
                });
            }
        };
        chunks_to_execute
    }

    /// Closes a data bus used for managing collectors and returns the first instance.
    ///
    /// # Arguments
    /// * `secn_instances` - A vector of secondary state machine instances.
    /// * `data_buses` - A vector of data buses with attached collectors.
    ///
    /// # Returns
    /// A vector of tuples containing the global ID, secondary state machine instance, and a vector
    /// of collectors for each instance.
    fn close_data_bus_collectors(
        &self,
        mut data_buses: DataBusCollectorCollection,
    ) -> Vec<(usize, Box<dyn BusDevice<u64>>)> {
        let mut collectors_by_instance = Vec::new();
        for (chunk_id, data_bus) in data_buses.iter_mut().enumerate() {
            if let Some(data_bus) = data_bus.take() {
                let mut detached = data_bus.into_devices(false);

                // As a convention the first element is the main collector the others are input generators
                let first_collector = detached.swap_remove(0);
                collectors_by_instance.push((chunk_id, first_collector.unwrap()));
            }
        }

        collectors_by_instance
    }
}

impl<F: PrimeField64, BD: SMBundle<F>> WitnessComponent<F> for ZiskExecutor<F, BD> {
    /// Executes the ZisK ROM program and calculate the plans for main and secondary state machines.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    ///
    /// # Returns
    /// A vector of global IDs for the instances to compute witness for.
    fn execute(&self, pctx: Arc<ProofCtx<F>>) -> Vec<usize> {
        // Process the ROM to collect the Minimal Traces
        timer_start_info!(COMPUTE_MINIMAL_TRACE);
        let min_traces_execution_mode = if self.asm_runner_path.is_none() {
            MinimalTraceExecutionMode::Emulator
        } else {
            MinimalTraceExecutionMode::AsmWithCounter
        };
        let min_traces = self.compute_minimal_traces(min_traces_execution_mode);
        timer_stop_and_log_info!(COMPUTE_MINIMAL_TRACE);

        timer_start_info!(COUNT);
        // Count the metrics for the Secondary SM instances
        let (main_count, secn_count) = if self.main_count.lock().unwrap().is_none() {
            self.count(&min_traces)
        } else {
            let main_count = self.main_count.lock().unwrap().take().unwrap();
            let secn_count = self.secn_count.lock().unwrap().take().unwrap();

            (main_count, secn_count)
        };
        timer_stop_and_log_info!(COUNT);

        // Plan the main and secondary instances using the counted metrics
        timer_start_info!(PLAN);
        let (mut main_planning, public_values) =
            MainPlanner::plan::<F>(&min_traces, main_count, Self::MIN_TRACE_SIZE);

        let mut secn_planning = self.sm_bundle.plan_sec(secn_count);
        timer_stop_and_log_info!(PLAN);

        // Configure the instances
        self.sm_bundle.configure_instances(&pctx, &secn_planning);

        // Assign the instances
        self.assign_main_instances(&pctx, &mut main_planning);
        self.assign_secn_instances(&pctx, &mut secn_planning);

        // Get the global IDs of the instances to compute witness for
        let main_global_ids =
            main_planning.iter().map(|plan| plan.global_id.unwrap()).collect::<Vec<_>>();
        let secn_global_ids = secn_planning
            .iter()
            .map(|plans| plans.iter().map(|plan| plan.global_id.unwrap()).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let secn_global_ids_vec = secn_global_ids.iter().flatten().copied().collect::<Vec<_>>();

        // Add public values to the proof context
        let mut publics = ZiskPublicValues::from_vec_guard(pctx.get_publics());
        for (index, value) in public_values.iter() {
            publics.inputs[*index as usize] = F::from_u32(*value);
        }
        drop(publics);

        // Update internal state with the computed minimal traces and planning.
        *self.min_traces.write().unwrap() = min_traces;
        *self.main_planning.write().unwrap() = main_planning;
        *self.secn_planning.write().unwrap() = secn_planning;

        [main_global_ids, secn_global_ids_vec].concat()
    }

    /// Computes the witness for the main and secondary state machines.
    ///
    /// # Arguments
    /// * `stage` - The current stage id
    /// * `pctx` - Proof context.
    /// * `sctx` - Setup context.
    /// * `global_ids` - Global IDs of the instances to compute witness for.
    fn calculate_witness(
        &self,
        stage: u32,
        pctx: Arc<ProofCtx<F>>,
        sctx: Arc<SetupCtx<F>>,
        global_ids: &[usize],
    ) {
        if stage != 1 {
            return;
        }

        for &global_id in global_ids {
            let (_airgroup_id, air_id) = pctx.dctx_get_instance_info(global_id);

            if MAIN_AIR_IDS.contains(&air_id) {
                let mut main_instances = self.main_instances.write().unwrap();

                let main_instance = main_instances
                    .entry(global_id)
                    .or_insert_with(|| self.create_main_instance(global_id));

                self.witness_main_instance(&pctx, main_instance);
            } else {
                let mut secn_instances = self.secn_instances.write().unwrap();

                let secn_instance = secn_instances
                    .entry(global_id)
                    .or_insert_with(|| self.create_secn_instance(global_id));

                match secn_instance.instance_type() {
                    InstanceType::Instance => {
                        self.witness_secn_instance(&pctx, &sctx, global_id, secn_instance)
                    }
                    InstanceType::Table => {
                        self.witness_table(&pctx, &sctx, global_id, secn_instance)
                    }
                }
            }
        }
    }

    /// Debugs the main and secondary state machines.
    ///
    /// # Arguments
    /// * `pctx` - Proof context.
    /// * `sctx` - Setup context.
    /// * `global_ids` - Global IDs of the instances to debug.
    fn debug(&self, pctx: Arc<ProofCtx<F>>, sctx: Arc<SetupCtx<F>>, global_ids: &[usize]) {
        for &global_id in global_ids {
            let (_airgroup_id, air_id) = pctx.dctx_get_instance_info(global_id);

            if MAIN_AIR_IDS.contains(&air_id) {
                MainSM::debug(&pctx, &sctx);
            } else {
                let secn_instances = self.secn_instances.read().unwrap();
                let secn_instance = secn_instances.get(&global_id).expect("Instance not found");

                secn_instance.debug(&pctx, &sctx);
            }
        }
    }

    fn gen_custom_commits_fixed(
        &self,
        pctx: Arc<ProofCtx<F>>,
        sctx: Arc<SetupCtx<F>>,
        check: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file_name = pctx.get_custom_commits_fixed_buffer("rom")?;

        let setup = sctx.get_setup(RomRomTrace::<usize>::AIRGROUP_ID, RomRomTrace::<usize>::AIR_ID);
        let blowup_factor =
            1 << (setup.stark_info.stark_struct.n_bits_ext - setup.stark_info.stark_struct.n_bits);

        gen_elf_hash(&self.rom_path, file_name.as_path(), blowup_factor, check)?;
        Ok(())
    }
}
