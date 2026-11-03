#import "../template.typ": adr, arch, course, ext, validation

== ADR-0036 — Parallel programming exercises <adr-0036>

*Status:* proposed. Refines #adr("0033"); extends #adr("0007") and #adr("0013"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("resource-management")[Resource management] and #arch("appendix-course-inventory")[Course inventory]; #adr("0002"), #adr("0008"), #adr("0021").

=== Context

An instructor asks for parallel programming exercises, which points to #course("LOG645") _Architectures de calculs parallèles_ (prerequisite #course("LOG200")). Its description covers the shared-memory, message-passing and hybrid models, speedup and Amdahl's law, and synchronization (semaphores, monitors, barriers). It names neither an API nor a language: C or C++ with #ext("openmp")[OpenMP] and #ext("mpi")[MPI] is assumed, to be confirmed with the coordinator (#arch("open-questions")[open question 24]).

The current design assumes sequential programs:

- A sandbox has one CPU (#adr("0033")). A parallel program on one CPU is correct but shows no speedup.
- Performance is measured in instructions (#adr("0007")). Summed over threads, that is the total work, which parallelism does not reduce: the measure cannot see a speedup.
- Wall-clock time on a shared VM varies by 5 to 30% (#adr("0007")).
- A data race may show up once in a hundred runs: one passing run proves nothing.
- C in the browser is still to evaluate (#adr("0008")), and threads in WebAssembly need a cross-origin isolated page.

=== Decision

*CPUs are an exercise limit.* An exercise declares `cpus`, 1 by default, up to a maximum set per judge VM. The sandbox receives that many dedicated cores. The judge counts a job of $n$ CPUs as $n$ of its concurrency slots, so the sizing and exam reservations of #adr("0021") hold in CPUs, not in jobs.

*Each model is a pack or an option.*

#table(
  columns: (3cm, 1fr),
  stroke: 0.5pt,
  [*Model*], [*How*],
  [Shared memory], [OpenMP and threads are options of the C and C++ packs (`-fopenmp`, `-pthread`), like the C standard (#adr("0013")).],
  [Message passing], [An MPI pack for C and C++ (#ext("open-mpi")[Open MPI]). Its run command starts $n$ processes inside the one sandbox; they communicate through shared memory, since the sandbox has no network.],
  [Hybrid], [The MPI pack with the OpenMP option.],
  [Several machines, GPUs], [Out of scope: a sandbox lives on one judge. GPU access under gVisor (#ext("nvproxy")[nvproxy]) passes calls through to the host driver, which gVisor then protects much less, and the judge VMs have no GPU.],
)

*A test case is run several times.* A parallel exercise sets how many runs a case needs, 3 by default, with the thread or process count varied between 1 and `cpus`. A case passes only if every run passes. For shared-memory C and C++, the exercise may add a run built with #ext("tsan")[ThreadSanitizer]: a reported race fails the case with the verdict `race`.

*Speedup is measured after the session.* Grading a speedup is optional per exercise (#arch("open-questions")[open question 24]). When asked for, it runs in the deferred measurement pass of #adr("0007"), on a judge with no other job: the program runs with 1 and with `cpus` workers on pinned cores, the median of several runs is kept, and the speedup is compared with the reference solution's, within a tolerance. Instruction counting is not used for these exercises.

*Visible tests run on the server.* A parallel exercise is never run in the browser (#adr("0008")).

=== Consequences

- A four-CPU job takes the capacity of four sequential jobs. A LOG645 exam reserves judges for its enrollment times `cpus`.
- The process limit of the MPI pack covers its processes and their threads, on top of gVisor's own (#arch("resource-management")[Resource management]).
- Judging a case several times multiplies its cost; the usage penalty of #adr("0022") charges it to its author like any other run.
- A race that never shows up in the runs and that ThreadSanitizer misses is still missed; the verdict lowers the odds, it does not prove absence.

#validation(id: "V-0036")[
  An OpenMP program with a data race on a shared counter fails at least one of its runs or the ThreadSanitizer run; the same program with a reduction passes. A two-process MPI exchange runs under gVisor; check whether Open MPI's single-copy shared memory, which relies on `process_vm_readv`, works or must be turned off. A job of four CPUs takes four slots of its judge. On an idle judge, the speedup of the same submission varies by less than the tolerance across ten measurement passes.
]
