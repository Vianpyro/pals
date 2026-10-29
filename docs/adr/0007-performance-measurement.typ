#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0007 — Deterministic and fair performance measurement <adr-0007>

*Status:* proposed; prototype to build, grading model to settle with the instructor. \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("resource-management")[Resource management] and #arch("load-and-performance")[Load and performance]; #adr("0001"), #adr("0002").

=== Context

An exercise may assess a solution's performance, not only its correctness. The goal is to assess the algorithm, not the language or the machine. Two obstacles stand in the way:

- *Noise.* Wall-clock time and CPU time vary with the VM's load, time stolen by the hypervisor, the cache and the processor frequency. On a shared VM, the spread reaches 5 to 30%, which is not enough to tell solutions apart.
- *Language.* The same algorithm is 10 to 100 times slower in Python than in Rust, and this factor varies with the operations. A fixed per-language multiplier therefore remains approximate.

The platform runs under #ext("gvisor")[gVisor] (#adr("0002")), in a VM where KVM is not guaranteed. `perf_event_open` is not available inside the sandbox.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Wall-clock or CPU time], [No overhead; simple.], [5 to 30% noise; language-dependent.],
  [Hardware counters (`perf`)],
  [Accurate, no slowdown.],
  [Rarely exposed in a VM; unavailable under gVisor.],

  [#ext("valgrind")[Valgrind] (`callgrind`)],
  [Deterministic instruction count.],
  [20 to 100$times$ slowdown; fragile with the JVM's JIT.],

  [#ext("qemu-user")[QEMU user mode] with an instruction-counting plugin],
  [Deterministic count; requires neither KVM nor hardware counters; 5 to 10$times$ slowdown.],
  [Compatibility with gVisor #ext("systrap")[Systrap] to be confirmed.],
)

=== Decision

*Measurement.* Performance is measured in *executed instructions*, under QEMU user mode (#ext("qemu-insn")[`qemu-x86_64 -plugin libinsn.so`]), not in time. For each runtime to behave deterministically:

- the student's code runs on a single thread; the runtime's own threads (the JVM's VM and JIT compiler threads) cannot be removed, so instructions are counted across all threads;
- inputs and the random seed are fixed;
- Python uses `PYTHONHASHSEED=0`;
- the JVM uses `-Xbatch`, a fixed-size heap and the serial collector (`-XX:+UseSerialGC`).

Counting is only active during the call to the student's function, thanks to a marker emitted by the harness. Reading the inputs, whose cost depends heavily on the language, is therefore excluded.

*Garbage collection* is not separated from the student's instructions: a solution that allocates more makes the collector work more, and that cost is legitimately its own. What matters is that collection is deterministic. With fixed inputs, a fixed heap and a serial collector, collections are triggered at the same points on every run. Python's reference counting is inline, and its cyclic collector is triggered by allocation counts. A collection that occurs at one input size but not the next creates a step in the count; the slope is therefore fitted over several sizes rather than computed from two.

*Neutralizing the language.* A student is only compared with a reference solution *in the same language*. The main criterion is *complexity*: several input sizes are measured, then the log-log slope of the instruction differences between sizes is computed, which removes the startup cost. This slope does not depend on the language. Two grading models remain to be settled with the instructor:

- *Complexity verdict*: check that the slope does not exceed the reference's, within a tolerance. A single reference per exercise is enough.
- *Full ranking*: rank first by complexity, then by the ratio to the instruction count of the same-language reference. This requires one reference per language and per exercise.

Memory follows the same logic: the peak memory, minus the runtime's baseline, is compared with the reference's.

*Two passes.* Measurement slows execution down. It is therefore never done while the student is waiting for an answer:

- *Correctness pass*: native execution, with generous time limits. It is the only one that responds during the exam.
- *Measurement pass*: it runs after the exam and only covers the *last* submission of each student for each exercise (one unique key per student–exercise pair). The queue is drained when the server is lightly loaded. Accuracy does not depend on it; the point is to leave the CPU to live judging.

The measurement queue is a lower-priority queue in #ext("postgresql")[PostgreSQL] (#adr("0001")). No new component is added.

#mermaid(
  "
  flowchart TB
    subgraph EXAM[During the exam]
      direction LR
      S[Submission] --> C[Correctness pass<br/>native, generous time limits]
      C --> V[Verdict to the student]
    end
    subgraph AFTER[After the exam, lower-priority queue]
      direction LR
      L[Last submission of each<br/>student for each exercise] --> Q[Measurement pass<br/>QEMU user mode, instruction budget]
      Q --> N[Instructions counted<br/>at several input sizes]
      N --> SL[Log-log slope]
      SL --> CMP[Compared with the reference<br/>in the same language]
    end
    C -.-> L
  ",
  document-context: true,
  width: 100%,
)

=== Consequences

- The execution limit of the measurement pass is an *instruction budget*. A generous wall-clock timeout remains in place as a safety net.
- Since the result is deterministic, the measurement pass can overload a machine, or run on another machine, without skewing the measurements.
- *Measurement data.* It is distinct from the tests. Large inputs are produced by an instructor-provided generator, with a seed chosen at measurement time, and the outputs are re-checked.
- *Pinned versions.* Runtime versions are pinned for a term. References are re-measured with the same images at each content release.
- *Appeals.* To be able to repeat an identical measurement, the source code, the image, the content release, the seed and the instruction count are kept.
- *Turnaround.* Computation time must be planned before grades are released: students $times$ exercises $times$ sizes $times$ duration of one measurement under QEMU.
- *Libraries.* For each exercise, the instructor specifies the allowed libraries, for example `sorted()` or `heapq`.
- *Questions to settle with the instructor:*
  - the choice of grading model;
  - whether code that does not pass all tests is measured;
  - which submission to measure if the last one fails while an earlier one passed;
  - how the result is presented to the student (for example "O(n log n), 1.8$times$ the reference").

#validation(id: "V-0007")[
  Measure an O(n²) sort and an O(n log n) sort in Python, Java and Rust, under QEMU and inside gVisor. Repeat each measurement 30 times, with and without #ext("stress-ng")[`stress-ng`] on the VM. Criteria:
  - a coefficient of variation below 0.1%;
  - a slope that separates the two sorts in all three languages;
  - ratios to the reference of the same order from one language to another;
  - the instruction count includes the JVM's threads other than the main one;
  - a slope that stays the same with the collector disabled (`gc.disable()` in Python, Epsilon GC in Java), so collection steps do not skew it.
]
