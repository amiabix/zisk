# Overview

> **Operators:** For configuration and deployment details, see [Distributed README](../../distributed/README.md)

---

## What is Distributed Proof Generation?

Distributed proof generation with ZisK is a synchronous effort between Coordinator and Workers to orchestrate proof generation across multiple machines. The core foundation lies in ZisK's modular state machine architecture, which enables horizontal scaling of proof workloads.

The Coordinator manages job lifecycles through a three-phase workflow (Contributions, Prove, Aggregate), coordinates worker pools via gRPC bidirectional streaming, and distributes tasks based on compute capacity.

## When to Use Distributed Proving

Distributed proving is most beneficial when you need to scale beyond the capabilities of a single machine. The system excels at handling large proofs that require more than 4-8 compute units, where the coordination overhead is justified by the significant performance gains. This is particularly valuable for production workloads that need to generate multiple proofs concurrently or handle complex computations that would overwhelm a single machine's memory and CPU resources.

For smaller proofs requiring fewer than 4 compute units, the overhead of distributed coordination often outweighs the benefits. In these cases, local proving provides faster iteration cycles and simpler debugging workflows. The distributed system is also overkill for occasional proof generation or when you have a single machine with sufficient resources to handle your workload.

## How ZisK's Architecture Enables Distribution

ZisK breaks down program execution into specialized state machines (Main, Arith, Binary, Memory, ROM etc.), each handling specific types of operations. This modular design allows Workers to process different segments of the computation independently, with each state machine generating its own witness traces. The Coordinator can then distribute these witness generation tasks across multiple Workers based on compute capacity, and then aggregates the results to produce the final proof.

## Architectural Components

### Coordinator

The Coordinator is the central orchestration operator that manages incoming proof requests and splits the work, based on required compute capacity, across distributed available workers.

**Core Responsibilities:**

1. **Job Orchestration**: Manages the complete lifecycle of proof generation jobs from creation to completion, tracking state progression and handling job-level error recovery.

2. **Worker Resource Management**: Maintains a pool of connected workers, tracks their compute capacity and health, and optimises resource allocation across the available worker pool.

3. **Task Distribution Engine**: Partitions proof generation work across multiple workers, assigns specific tasks based on worker capacity, and tracks progress of distributed computations.

4. **Phase Orchestration**: Manages the three-phase proof generation workflow, coordinates phase transitions, selects aggregators, and ensures proper synchronisation between all workers.

**Operator Characteristics:**
- **Stateless Design**: Maintains job state but doesn't perform computations
- **High Availability**: Single point of coordination with fault tolerance
- **Scalable Architecture**: Can manage hundreds of workers simultaneously
- **gRPC Interface**: Exposes RESTful API for job submission and monitoring

### Worker

The Worker is the computational execution operator responsible for performing the actual cryptographic proof generation work, executing the three-phase proof generation.

**Core Responsibilities:**

1. **Proof Computation Engine**: Executes the actual proof generation work, performing challenge generation, individual proof generation and proof aggregation when selected as aggregator.

2. **Dual Operator Modes**: Operates in two modes - gRPC Worker (rank 0) maintains direct communication with the coordinator, while MPI Worker (rank 1+) uses high-performance cluster communication for distributed computing environments.

3. **ProofMan Integration**: Leverages the ProofMan library for actual proof generation, utilising GPU acceleration and optimised algorithms to handle all three phases of the proof generation process efficiently.

## How the System Works

### Communication Architecture

```
┌─────────────────┐    gRPC Streaming     ┌─────────────────┐
│                 │◄─────────────────────►│                 │
│   Coordinator   │                       │   Worker (0)    │
│                 │                       │                 │
└─────────────────┘                       └─────────────────┘
                                                           │ MPI
                                                           ▼
                                                  ┌─────────────────┐
                                                  │   Worker (1)    │
                                                  └─────────────────┘
                                                           │ MPI
                                                           ▼
                                                  ┌─────────────────┐
                                                  │   Worker (N)    │
                                                  └─────────────────┘
```

### Proof Generation Workflow

The process of generating a proof proceeds as follows:

1. The Coordinator starts on a host and listens for incoming proof requests.
2. Worker nodes connect to the Coordinator, registering their compute capacity and availability.
3. When a proof generation request is received, the Coordinator splits the work across multiple Workers according to the requested compute capacity. A proof generation job is divided into three phases:
   - **Partial Contributions**: Each Worker computes its partial challenges.
   - **Prove**: Workers compute the global challenge and generate their respective partial proofs.
   - **Aggregation**: A designated Worker aggregates all partial proofs and produces the final proof for the client.
4. The Coordinator collects the final proof and returns it to the client.

### Key Concepts

**Worker Selection**: The Coordinator selects Workers based on their reported compute capacity and availability. When a proof request is received, the Coordinator evaluates the required compute capacity and selects Workers sequentially from the pool of available Workers until the total capacity requirement is met. When a worker is assigned to a job, it is marked as busy and will not receive new tasks until it completes the current job.

**Aggregator Selection**: The first Worker to send its partial proof to the Coordinator is selected as the Aggregator to perform the aggregation of all partial proofs into the final proof. The other Workers are marked as available again after sending their partial proofs.

## Quick Start Example

Here's a complete working example to get you started:

{{#include ../_snippets/distributed/quick-start.md}}

## Three-Phase Proof Generation Workflow

The distributed proof generation process is divided into three sequential phases, each building upon the previous one:

### Phase 1: Contributions (Challenge Generation)

**Purpose**: Generate cryptographic challenges for the proof
**Duration**: Typically 20-40% of total proof time
**Parallelization**: Fully parallel across workers

**Process**:
1. Coordinator distributes computation across selected workers based on capacity
2. Each worker generates challenges for their assigned work partition
3. Workers use `ProofMan::generate_proof_from_lib()` with `ProvePhase::Contributions`
4. Returns `Vec<ContributionsInfo>` containing generated challenges
5. Coordinator aggregates challenges from all workers

**Example Output**:
```
[INFO] Worker worker-1: Starting contributions phase for job job_123
[INFO] Worker worker-1: Generated 1024 challenges in 45.2s
[INFO] Coordinator: Aggregated challenges from 3 workers
```

### Phase 2: Prove (Partial Proofs Generation)

**Purpose**: Generate individual proofs using challenges from Phase 1
**Duration**: Typically 50-70% of total proof time
**Parallelization**: Fully parallel across workers

**Process**:
1. All workers receive the complete aggregated challenge set
2. Each worker generates partial proofs for their designated portion
3. Workers compute global challenges and produce `Vec<AggProofs>`
4. Partial proofs are sent back to coordinator

**Example Output**:
```
[INFO] Worker worker-1: Starting prove phase with 3072 challenges
[INFO] Worker worker-1: Generated partial proof in 128.7s
[INFO] Coordinator: Received partial proof from worker-1
```

### Phase 3: Aggregate (Final Proof Assembly)

**Purpose**: Combine all partial proofs into a single final proof
**Duration**: Typically 10-20% of total proof time
**Parallelization**: Single worker (aggregator)

**Process**:
1. **Aggregator Selection**: First worker to complete Phase 2 becomes aggregator
2. Designated worker aggregates all partial proofs into final proof
3. Other workers marked as available after sending partial proofs
4. Final proof triggers completion webhooks and cleanup

**Example Output**:
```
[INFO] Coordinator: Selecting worker-2 as aggregator
[INFO] Worker worker-2: Aggregating 3 partial proofs
[INFO] Worker worker-2: Final proof generated in 23.1s
[INFO] Coordinator: Job job_123 completed successfully
```


## Performance Characteristics

The distributed system demonstrates excellent scaling characteristics, with performance scaling linearly with the number of workers up to the point where coordination overhead becomes significant. In practice, this means you can expect approximately 1.8x speedup with 2 workers, 3.5x with 4 workers, 6.8x with 8 workers, and 13.2x with 16 workers for large proofs.

Memory requirements scale proportionally with the number of workers. Each worker typically needs 8-16GB of RAM for large proofs, while the coordinator requires 2-4GB. For large computations, workers may need 20GB or more of shared memory to handle the proof generation process efficiently.

### Resource Optimization

The coordinator implements an intelligent worker selection algorithm that considers reported compute capacity and availability when distributing work. Workers are selected sequentially until the total capacity requirement is met, and they are marked as busy during execution to prevent double-allocation. The system uses a round-robin allocation strategy to ensure even distribution of work units across available workers.

The system supports two execution modes. In standard mode, work is distributed across all available idle workers, maximizing parallelization. Simulation mode allows a single worker to simulate multiple nodes for testing purposes, which is particularly useful during development when you want to validate distributed behavior without multiple physical machines.

## Common Use Cases

The distributed proving system is particularly well-suited for several key scenarios that benefit from horizontal scaling:

- **High-Throughput Proof Generation**: Ideal for blockchain applications that need to generate multiple proofs concurrently. With a setup of 4-8 workers, each providing 4-8 compute units, you can achieve 3-6x throughput improvement over a single-machine setup. This is especially valuable for applications that need to process many transactions or computations in parallel.

- **Large Proof Computation**: Becomes practical when dealing with complex computations requiring 16 or more compute units. By distributing this work across 4 workers with 4 compute units each, the proof can complete in approximately 4x less time than a single machine, making previously impractical computations feasible.

- **Production Service**: Deployments benefit from the system's ability to handle varying loads consistently. With 8-16 workers and auto-scaling based on demand, the system can maintain consistent performance even as client requests fluctuate throughout the day.

- **Development and Testing**: Scenarios are well-supported through simulation mode, where a single worker can simulate multiple nodes. This allows developers to validate distributed system behavior without requiring multiple physical machines, making the development process more accessible and cost-effective.

## See Also

- [Local Development Guide](./local_development.md) for hands-on setup and testing
- [Docker Deployment](./docker_deployment.md) for production deployment
- [Distributed README](../../distributed/README.md) for operator configuration and troubleshooting

## MPI Integration for Multi-Node Clusters

### Worker Node Hierarchy

**WorkerNodeGrpc** (Rank 0):
- Communicates with coordinator via gRPC
- Receives task assignments and reports results
- Coordinates with local MPI ranks

**WorkerNodeMpi** (Rank 1-N):
- Receives tasks via MPI broadcast from Rank 0
- Executes computation in parallel
- No direct coordinator communication

### MPI Coordination Flow

```rust
// Rank 0 broadcasts task to all local ranks
let mut serialized = borsh::to_vec(&(JobPhase::Contributions, job_id, phase_inputs, options)).unwrap();
self.proofman.mpi_broadcast(&mut serialized);
```

**Process**:
1. Coordinator sends task to Worker Rank 0 via gRPC
2. Rank 0 serializes task data and broadcasts via MPI
3. All ranks execute computation phase in parallel
4. Only Rank 0 reports results to coordinator

## Communication Protocols

### gRPC Bidirectional Streaming

**Worker → Coordinator Messages**:
- `WorkerRegisterRequest`: Initial registration with compute capacity
- `ExecuteTaskResponse`: Task completion results (challenges, proofs, final proofs)
- `HeartbeatAck`: Keep-alive acknowledgments
- `WorkerError`: Error reporting

**Coordinator → Worker Messages**:
- `ExecuteTaskRequest`: Task assignments (contribution, prove, aggregate)
- `Heartbeat`: Keep-alive messages
- `JobCancelled`: Job cancellation notifications
- `Shutdown`: Graceful shutdown requests

### Administrative API

Unary RPC methods for monitoring and control:
- `HealthCheck()`: Basic service health status
- `SystemStatus()`: Overall system metrics
- `JobsList()`: Current and historical jobs
- `WorkersList()`: Connected workers and states
- `LaunchProof()`: Start new proof job

## Job Lifecycle Management

### Job States

```rust
pub enum JobState {
    Created,
    Running(JobPhase),  // Contributions, Prove, or Aggregate
    Completed,
    Failed,
}
```

### Lifecycle Flow

1. **Job Creation**: `create_job()` allocates workers and resources
2. **Validation**: `pre_launch_proof()` validates parameters
3. **Execution**: Three-phase workflow with state transitions
4. **Completion**: `post_launch_proof()` handles cleanup and notifications

### Error Handling & Recovery

**Fault Tolerance Features**:
- Worker failure detection via heartbeat mechanism
- Task reassignment on worker failures
- Automatic reconnection with state preservation
- Capacity validation prevents resource exhaustion

**Recovery Strategies**:
- Failed workers automatically marked as unavailable
- Jobs can continue with remaining workers if capacity allows
- Graceful degradation and error propagation

## Performance Characteristics

**Scalability**:
- Linear performance scaling with worker count
- Efficient work distribution via round-robin allocation
- MPI coordination minimizes communication overhead

**Resource Optimization**:
- Capacity-aware scheduling optimizes resource utilization
- Dynamic worker allocation based on availability
- Memory-efficient serialization for large data transfers

**Monitoring**:
- Real-time job and worker status tracking
- Comprehensive metrics and logging
- Webhook notifications for job completion

This architecture enables ZisK to scale proof generation across large distributed clusters while maintaining coordination, fault tolerance, and optimal resource utilization.
