

# **A Deny-by-Default Architecture for Secure AI Agents: Design and Implementation with Rust and WebAssembly**

## **I. Architectural Mandate: The Deny-by-Default, Capability-Based Agent**

### **A. The Foundational Flaw of Privileged, "Full-Access" Agents**

The rapid evolution of Large Language Models (LLMs) has introduced a new class of "agentic" software capable of autonomous planning and execution. A common but fundamentally insecure architectural pattern grants these AI agents privileged, "full-access" to the host system's resources, such as the filesystem, network, and system commands. This design, often implemented by having the LLM generate code (e.g., Python) that is subsequently executed in a full-permission environment, presents catastrophic security risks.1  
This privileged model operates on a flawed assumption: that the LLM is a trusted component. In reality, the LLM itself is a vast, untrusted attack surface. Security vulnerabilities specific to this paradigm include:

1. **Prompt Injection:** A malicious user or, more subtly, malicious *data* consumed by the agent (e.g., from a webpage or email) can "trick the LLM into making unauthorized API calls".2 This can lead to data deletion, data leakage, or the execution of unintended commands.  
2. **Indirect Execution and Intent Manipulation:** An LLM-driven application may be manipulated by the *data it processes*, leading to indirect execution of hostile commands.3

The primary takeaway is that the security boundary for an AI agent *cannot* be placed at the point of user input; traditional input sanitization is insufficient when the processor *itself* (the LLM) is susceptible to manipulation. The security boundary *must* be placed at the final point of action execution.  
This report details an architecture that achieves this by rejecting the full-access model. The foundational principle is that the agent's execution environment must be a "deny-by-default" sandbox. WebAssembly (Wasm), with its binary instruction format, stack-based virtual machine, and rigorous sandboxing, is the ideal technology for enforcing this boundary.4

### **B. Defining the "Human-on-the-Loop," Capability-Based Model**

This architecture is founded on a set of core security principles designed to address the risks of privileged agents and fulfill the user's objective of a secure, "human-on-the-loop" or "human-on-the-loop" system.

1. **Principle 1: Deny-by-Default:** The Wasm agent (guest) possesses *zero* inherent permissions upon instantiation. It is isolated in a secure sandbox and cannot access the filesystem, network, environment variables, or any other host resource unless a capability is explicitly and granularly granted by the host.6  
2. **Principle 2: Capability-Based Security:** This architecture fully embraces the capability-based security model, which is a core design principle of the WebAssembly System Interface (WASI).7 All access to external resources is mediated by "capabilities"—unforgeable, opaque handles or tokens (represented as Wasm resources) that are defined and vended by the host. The agent cannot forge these handles or escalate its privileges; it can only request operations on the specific resources to which it has been granted access.8  
3. **Principle 3: The Host as Sole Authority:** The Rust host application serves as the sole authority, or "reference monitor," for the system. It defines the API, implements all capability logic, validates all agent requests, manages the lifecycle of all Wasm instances, and retains absolute control over system resources.10  
4. **Principle 4: "Human-on-the-Loop":** The system is architected to escalate permission requests to a human operator. When an agent attempts to perform an action for which it has not been granted a capability (e.g., access a new file path or a new network domain), the host control loop pauses execution and prompts the human user for explicit consent. This model, demonstrated in practice by systems like Microsoft's wassette 11, transforms security from a static configuration into a dynamic, interactive, and auditable process.

## **II. The Secure Foundation: Host Runtime Integration and Sandboxing**

### **A. Analysis of Wasm Runtimes (Answering Q1)**

The choice of Wasm runtime is the first critical implementation decision. The runtime is responsible for JIT/AOT compilation, instantiation, and enforcement of the sandbox. The analysis focuses on the three leading standalone runtimes: wasmtime, wasmer, and wazero.

* **wasmtime:**  
  * **Governance & Security:** wasmtime is the flagship project of the Bytecode Alliance, a non-profit organization backed by Mozilla, Fastly, and others.12 Its development is "strongly focused on correctness and security" 13, building on Rust's runtime safety guarantees. It undergoes 24/7 fuzzing by Google's OSS Fuzz 13 and integrates best-practice, defense-in-depth mitigations for side-channel attacks like Spectre.13 This directly addresses high-level security concerns.14  
  * **Performance & Memory:** It is built on the optimizing Cranelift code generator, enabling fast JIT compilation and near-native execution speeds.12 Crucially, it demonstrates a significantly lower memory footprint, with one analysis showing a 12 MB max resident size compared to 24 MB for wasmer.12  
  * **Standards:** As the Bytecode Alliance's reference implementation, wasmtime has first-class, production-ready support for the latest Wasm standards, most importantly **WASI Preview 2** and the **Wasm Component Model**.15 This is a decisive advantage, as the Component Model is central to the recommended API and data-marshalling strategy.  
* **wasmer:**  
  * **Governance & Features:** wasmer is an alternative runtime, also written in Rust and easy to embed.12 It is run by a corporation and is associated with the wapm package manager.12  
  * **Performance & Memory:** It offers performance comparable to wasmtime but with a noted higher memory overhead.12  
* **wazero:**  
  * This runtime is written in Go. While notable for its zero-dependency footprint in the Go ecosystem, it is not a suitable choice for a host application written in Rust.

---

**Table 1: Wasm Runtime Comparative Analysis**

| Feature | wasmtime | wasmer | wazero |
| :---- | :---- | :---- | :---- |
| **Primary Language** | Rust | Rust | Go |
| **Governance** | Bytecode Alliance (Non-Profit) 12 | Wasmer, Inc. (Corporation) 12 | Community (Go-based) |
| **Security Focus** | **Exceptional.** 24/7 fuzzing, Spectre mitigations, strong Rust safety guarantees 13 | High (Rust-based) | High (Go-based) |
| **Performance** | Near-native (Cranelift JIT) 12 | Near-native 12 | High (JIT or Interpreter) |
| **Memory Overhead** | **Low** (e.g., 12 MB resident) 12 | Medium (e.g., 24 MB resident) 12 | Low |
| **Component Model** | **Reference Implementation.** 15 | Supported | Not a primary focus |

---

**Recommendation:** wasmtime is the unequivocal choice for this architecture. Its superior security-first design, lower memory footprint, non-profit governance, and role as the reference implementation for the Wasm Component Model make it the ideal foundation.

### **B. Host Integration: The Engine, Store, and Instance Lifecycle (Answering Q1)**

Properly managing the wasmtime object lifecycle is critical for security, performance, and resource management. The primary types are Engine, Module, Store, and Instance.16

* The Engine (Global, Long-Lived):  
  The Engine is the "root context" for all Wasm operations.17 It is a thread-safe (Send+Sync) compilation environment that holds the JIT compiler (Cranelift) and global configuration. An Engine should be created once at host application startup and shared across all threads.  
  let engine \= Engine::default(); 16  
* The Module (Global, Long-Lived):  
  The Module is the compiled, executable representation of the agent's .wasm file.17 This compilation step (Module::from\_file(\&engine,...) 16\) is computationally expensive and should also be performed once at startup. The resulting Module can be cloned cheaply and used to instantiate many different agents.  
* The Store (Scoped, Short-Lived):  
  The Store is the most critical component for resource and security isolation. It holds all instance-specific state, including linear memory, tables, globals, and handles to host objects.17  
  A critical point documented by wasmtime is that the Store is "intended to be a short-lived object" and, crucially, has **"No form of GC"** (Garbage Collection).19 Creating an unbounded number of instances within a single, long-lived Store *will* result in a memory leak, as all instance-related memory is only reclaimed when the Store itself is dropped.19 This is a fatal flaw for an agentic system that may spawn thousands of tasks. Reference cycles between host state and Store-bound objects can also create memory leaks.22  
  Architectural Mandate: The only correct and safe pattern is to create a new Store for every discrete agent task.  
  let mut store \= Store::new(\&engine, MyTaskState {... }); 16  
  When the task is complete, the store variable goes out of scope and is dropped. This instantly and deterministically reclaims all memory associated with that task (linear memory, tables, etc.), providing a perfect, low-cost, task-level memory sandbox.  
* The Instance (Scoped, Short-Lived):  
  The Instance is the running Wasm module.17 It is created from a Module and a Store.16 Its lifecycle is bound entirely to the Store.19

### **C. Configuring the "Zero-Trust" Sandbox (Answering Q5)**

The "deny-by-default" principle is implemented by configuring the Engine and Store's WASI context.

* wasmtime::Config (Engine-level):  
  This struct is used to configure the Engine before its creation.  
  1. Resource Limiting: To prevent Denial of Service (DoS) attacks from malicious or buggy Wasm (e.g., infinite loops, "billion laughs" attacks) 14, we will enable "fuel" consumption. Fuel is a measure of computation.  
     config.consume\_fuel(true); 14  
     Before executing an agent task, the host will "add fuel" to the Store. If the agent consumes all its fuel, it traps, safely terminating the task.  
     store.add\_fuel(1\_000\_000)?;  
  2. Asynchronous Support: To allow host-defined capabilities (like network I/O) to be non-blocking, async support is enabled.  
     config.async\_support(true); 24  
* wasmtime\_wasi::WasiCtxBuilder (Store-level):  
  This is the primary tool for defining the agent's sandboxed environment.26 By default, it is highly restrictive, but we will make it explicitly zero-trust.  
  1. **Denying Environment Variables:** The builder does *not* inherit environment variables from the host by default.27 We *must ensure* WasiCtxBuilder::inherit\_env() is *never* called.  
  2. Denying Networking: WASI Preview 2's builder allows TCP and UDP by default, but denies all addresses.26 This is insufficient for a true "deny-by-default" posture. We will explicitly disable all raw socket access:  
     let mut builder \= WasiCtxBuilder::new();  
     builder.allow\_tcp(false);  
     builder.allow\_udp(false); 26  
     We must also ensure WasiCtxBuilder::inherit\_network() is never called.25 This forces the agent to use our custom, host-defined, capability-based API for any network requests, as mandated by the architecture.  
  3. **Denying Process Execution:** Standard WASI does not include fork or exec capabilities. By not providing any custom process-execution functions, this attack vector is denied by default.  
* Granting Specific Rights: The WASI "Pre-open" (Answering Q5):  
  The only privilege we will grant the agent via standard WASI is restricted filesystem access to a single, non-sensitive directory. This is achieved via "pre-opens".28  
  let workspace\_dir \= std::fs::File::open("./workspace")?;  
  builder.preopened\_dir(workspace\_dir, "/"); 29  
  This maps the host's ./workspace directory to the guest's root (/) directory.30 When the agent's internal code (e.g., Rust's std::fs::read\_to\_string("project.txt")) executes, the WASI runtime translates this to a read of ./workspace/project.txt. Any attempt to access /etc/passwd or ../ outside the workspace will fail at the WASI level, as it is outside the pre-opened directory's handle.32

## **III. The Core Contract: Designing the Secure Capability API (Answering Q2)**

### **A. The Ergonomics-Security Tension: "Command-like" vs. "Capability-like"**

The most critical component of this architecture is the API exposed by the host to the Wasm guest. This API must resolve the tension between two competing goals:

1. **Ergonomics ("Command-like"):** An API that is ergonomic for an LLM to "think" about. LLMs are trained on natural language and high-level code, favoring semantic, descriptive functions 2 like fn read\_file(path: String) \-\> String.33  
2. **Security ("Capability-like"):** An API that is easy to secure, following the principle of least privilege.

The "command-like" approach (fn read\_file(path: String)) is a classic "Confused Deputy" vulnerability. It forces the host to perform complex, brittle, and error-prone string validation on *every single call* to prevent path traversal (../) 34 or other injection attacks.3 A single mistake in this validation logic (e.g., failing to account for Unicode or \\0 bytes) is a critical sandbox escape.  
The "capability-like" approach, which is the foundation of WASI 7 and secure system design 37, solves this. It uses "Handles" 8—opaque, unforgeable tokens (represented as Wasm resources) that grant rights to a *specific* object.  
Recommended Design Pattern:  
The API will not expose fn read\_file(path: String). Instead, it will be designed around resource handles, inspired by WASI's own file-descriptor model:

1. fn open\_workspace\_dir() \-\> Result\<DirHandle\>: This is the *only* initial filesystem capability the host grants. DirHandle is an opaque resource.  
2. fn open\_file(dir: \&DirHandle, path: String) \-\> Result\<FileHandle\>: Opens a file *relative to* a granted DirHandle.  
3. fn read\_to\_string(file: \&FileHandle) \-\> Result\<String\>: Reads from a granted FileHandle.  
4. fn write\_file(dir: \&DirHandle, path: String, contents: String) \-\> Result\<()\>: Writes a file *relative to* a granted DirHandle.  
5. fn list\_dir(dir: \&DirHandle, path: String) \-\> Result\<List\<String\>\>: Lists entries *relative to* a granted DirHandle.

This capability-based design *solves* the ergonomics-security tension. The LLM can still "think" in ergonomic terms ("I need to read project.txt from the workspace"), but the *action* it must generate is, by necessity, secure and auditable. The LLM's plan must be: let ws \= open\_workspace\_dir(); let file \= open\_file(ws, "project.txt"); let contents \= read\_to\_string(file);. This design forces the LLM to follow a secure-by-default workflow. It *cannot* forge a DirHandle and thus cannot operate outside the directories it has been explicitly granted.

### **B. Host-Side Security: A Framework for Paranoid Input Validation (Answering Q2)**

This architecture provides defense-in-depth. Even with a capability-based API, all *string inputs* from the guest (like the path argument) must be treated as hostile.34 The *host-side Rust implementation* of fn open\_file(dir: \&DirHandle, path: String) must perform its own redundant, paranoid validation:

1. Retrieve the *host-side* canonical path associated with the DirHandle from the host's internal state.  
2. Sanitize the guest-provided path string. It must *not* be an absolute path (e.B., start with /) and must *not* contain .. components.35  
3. Securely join the DirHandle's base path and the sanitized guest path.  
4. Canonicalize the resulting joined path (e.g., using std::fs::canonicalize).  
5. **Critically:** Check that this final, canonicalized path is *still a child* of the DirHandle's base path. This prevents subtle attacks like symlink following (if not desired) or other canonicalization-based bypasses.  
6. If any check fails, the function must return a specific, descriptive Err (e.g., Err("PermissionDenied: Path traversal detected")). This error message is not just for security; it is *data* that will be fed back to the LLM to help it self-correct.38

This validation is non-negotiable and provides a secondary security boundary against a compromised Wasm module or a tricked LLM.3

### **C. Implementation: "Classic" vs. "Modern" API Definition**

There are two primary methods for implementing this host-guest API in wasmtime.

1. Classic (Linker::func\_wrap):  
   This is the low-level, "core Wasm" approach. The host uses wasmtime::Linker to manually define each imported function.39  
   Rust  
   linker.func\_wrap(  
       "my-api",  
       "read\_file\_safely",

|mut caller: Caller\<'\_, HostState\>, ptr: i32, len: i32| \-\> i32 {  
//... manual memory operations to read the string...  
//... manual memory operations to write the result...  
}  
)?;  
\`\`\`  
This method is cumbersome and highly error-prone. It requires manual, unsafe memory management on both the host and guest sides to pass complex data like strings, which must be encoded as pointers (i32) and lengths (i32).40

2. Modern (Wasm Component Model / WIT):  
   This is the high-level, modern solution. The API contract is defined in a language-agnostic WIT (WebAssembly Interface Type) file.41  
   The wasmtime::component::bindgen\! macro 42 then reads this WIT file and auto-generates all the low-level boilerplate code. The host simply implements a high-level, idiomatic Rust trait:  
   Rust  
   // bindgen\! generates this trait from the WIT file  
   impl Host for MyState {  
       async fn read\_file\_safely(\&mut self, path: String)  
           \-\> Result\<String, String\>  
       {  
           //... safe, idiomatic Rust logic...  
       }  
   }

   This is the same mechanism demonstrated in the wasmtime documentation's bindgen examples.42

**Recommendation:** The Wasm Component Model is overwhelmingly superior. It is type-safe, high-level, and completely eliminates the complex, insecure, and error-prone manual data marshalling, which is the single greatest source of flaws in Wasm FFI.44

## **IV. Secure Data Marshalling: Crossing the Host-Guest Boundary (Answering Q4)**

### **A. The Core Wasm Limitation: A Numeric-Only World**

The core WebAssembly virtual machine only supports four data types: i32, i64, f32, and f64.45 Passing complex, high-level types like strings, structs, byte arrays, or JSON objects requires an "Application Binary Interface" (ABI)—a convention for a-laying out this data in Wasm's linear memory.47

### **B. Strategy 1: The "Serialize-and-Copy" Model (Serde \+ Linear Memory)**

This is the traditional, manual approach to Wasm FFI.

* **Mechanism:**  
  1. The Wasm guest allocates a buffer in its linear memory (e.g., using Vec::with\_capacity).  
  2. The guest serializes its complex data (e.g., a struct Request) into a known format, typically JSON using serde\_json.48  
  3. The guest calls the imported host function, passing the *pointer* (an i32) and *length* (an i32) of this JSON string in the buffer.40  
  4. The Rust host receives the ptr and len, uses Memory::read to *copy* the JSON bytes *out* of the Wasm linear memory.  
  5. The host de-serializes the JSON bytes into a host-side Rust struct.  
  6. The entire process is reversed for returning data, requiring the guest to expose a malloc function for the host to write into.47  
* **Advantages:**  
  * Uses the well-understood serde library.50  
* **Disadvantages:**  
  * **Performance:** Extremely slow. It involves two serializations and two de-serializations (Guest Struct \-\> Guest JSON \-\> Host JSON \-\> Host Struct), plus a full memory copy.51  
  * **Ergonomics:** Incredibly complex and error-prone, requiring manual memory management, pointer arithmetic, and unsafe code on both sides.  
  * **Security:** This complex, unsafe pointer logic is a large attack surface.

### **C. Strategy 2 (Recommended): The Wasm Component Model and WIT**

The Wasm Component Model is a technology designed specifically to solve this problem.15

* **Mechanism:**  
  1. **WIT (WebAssembly Interface Type):** The API contract is defined in a high-level IDL file (.wit).54 This file defines the functions and their parameters using high-level types like string, list\<u8\>, result, and custom resource types.  
  2. **wit-bindgen & cargo component:** These tools 44 read the wit file and *auto-generate* all the low-level "glue code" for both the host and the guest.41  
  3. **Canonical ABI:** The generated code automatically (and efficiently) serializes and copies the data across the boundary using the standard "Canonical ABI," which is a highly efficient binary format, not JSON.54  
* **Example agent.wit file:**  
  Code snippet  
  // e.g., in wit/agent.wit  
  package my-org:agent;

  world agent-api {  
    // Define our capability handles as opaque resources  
    resource file-handle;  
    resource dir-handle;

    // Define host-provided imports (the Capability API)  
    import open-workspace-dir: func() \-\> result\<dir-handle, string\>;  
    import read-to-string: func(file:, file-handle) \-\> result\<string, string\>;  
    import write-file: func(dir: dir-handle, path: string, contents: string) \-\> result\<(), string\>;  
    import list-dir: func(dir: dir-handle, path: string) \-\> result\<list\<string\>, string\>;

    // Define guest-provided exports (the Agent's entry point)  
    export run-task: func(task-description: string) \-\> result\<string, string\>;  
  }

* **Advantages:**  
  * **Ergonomics:** Trivial. The host and guest interact using their native, idiomatic types (String, Result, custom structs).41 All pointer-passing and memory management is hidden.  
  * **Type Safety:** The API contract is statically enforced at compile time for both host and guest.  
  * **Performance:** Significantly faster than JSON serialization, as it uses an optimized binary ABI.  
  * **Language Agnostic:** The guest could be re-written in Go or Python, and as long as it targets the same WIT file, the host would not need to change.53

### **D. Security: Shared Memory vs. "Serialize-and-Copy" (Answering Q4)**

Wasm's linear memory is *not* safe from *internal* memory vulnerabilities. If the guest is written in a language like C, or uses unsafe Rust, it can be vulnerable to buffer overflows or use-after-free bugs within its *own* memory.56  
If the host and guest were to use the Wasm "shared-memory" proposal (which requires multi-threading), a buffer overflow vulnerability in the guest could *directly* corrupt host memory, completely breaking the sandbox.  
Therefore, the "serialize-and-copy" approach is fundamentally safer. Both Strategy 1 (Serde/JSON) and Strategy 2 (Component Model) *use a copy-based approach*. The wasmtime runtime copies data *across* the Wasm boundary, from the guest's linear memory to the host's memory.58 This maintains the hard isolation barrier.  
**Recommendation:** The Wasm Component Model (Strategy 2\) is the clear winner. It provides the security of a copy-based boundary with the high-level ergonomics of a native function call. **The Wasm shared-memory proposal should be explicitly avoided for this high-security architecture.**  
---

**Table 2: Data Marshalling Strategies (Serde/JSON vs. Component Model)**

| Strategy | Developer Ergonomics | Performance | Type Safety | Security |
| :---- | :---- | :---- | :---- | :---- |
| **1\. Serde/JSON \+ Linear Memory** | Very Poor. Requires manual, unsafe memory management.47 | Very Slow. Double serialization \+ copy.51 | Poor. Relies on ptr and len (i32) only. | Poor. Large, complex, unsafe attack surface. |
| **2\. Wasm Component Model** | **Excellent.** Uses native Rust types (String, Result).41 | **High.** Uses efficient Canonical ABI \+ copy.54 | **Excellent.** Statically enforced by WIT.44 | **High.** Secure "by-construction," minimal unsafe code. |

---

## **V. The Agent: Guest-Side Implementation and Logic (Answering Q3)**

### **A. Compiling the Agent: The wasm32-wasip2 Target**

To build a Wasm module that conforms to the Component Model and can consume the WIT-defined API, the agent's Rust project must be compiled with the correct target and tooling.

1. Toolchain Setup: The standard wasm32-wasi (or "wasip1") target 59 is not sufficient. The modern "wasip2" target, which is built on the Component Model, is required.  
   $ rustup target add wasm32-wasip2 13  
2. cargo-component: This community tool extends cargo to understand WIT files and Component Model compilation.  
   $ cargo install cargo-component 55  
3. Building: The agent is built using a new command, which automatically invokes wit-bindgen and builds a Component-native .wasm file.  
   $ cargo component build \--release 41

This process is distinct from compiling for the browser (wasm32-unknown-unknown) 60 or for older WASI (wasm32-wasi).30

### **B. Guest-Side Bindings: Calling Host Capabilities**

The guest module must declare its *imports* to call the host's capability API.

* Classic (extern "C"):  
  If using the "Classic" API, the guest's Rust code must manually define an extern "C" block with a \#\[link(wasm\_import\_module \= "...")\] attribute.62  
  Rust  
  \#\[link(wasm\_import\_module \= "my-org:agent")\]  
  extern "C" {  
      \#\[link\_name \= "read-to-string"\]  
      fn read\_to\_string(handle\_ptr: \*const u8,...) \-\>...;  
  }  
  // This requires unsafe code to call  
  unsafe { read\_to\_string(...) }

  This is unsafe, brittle, and requires manual memory marshalling.40  
* Modern (Component Model):  
  This approach is vastly superior. The agent's src/lib.rs file simply includes the wit\_bindgen::generate\! macro, pointing to the same agent.wit file used by the host.41  
  Rust  
  // In the guest's src/lib.rs  
  wit\_bindgen::generate\!({  
      path: "../wit/agent.wit", // Path to the WIT file  
      world: "agent-api",  
  });

  // This macro generates a module, typically named after the WIT file  
  // or package. We'll also define the \*export\* (our entry point).  
  struct MyAgent;  
  impl Guest for MyAgent { // 'Guest' trait is generated by bindgen  
      fn run\_task(task\_description: String) \-\> Result\<String, String\> {  
          //... agent logic here...  
      }  
  }  
  export\!(MyAgent); // Exports the agent for the host to call

  The bindgen\! macro automatically generates a crate::imports module (or similar, based on the WIT structure) containing safe, high-level Rust functions for *all* the imports. The agent's logic can then call them safely:  
  Rust  
  // Inside run\_task...  
  use crate::imports::{open\_workspace\_dir, read\_to\_string, write\_file};

  let dir\_handle \= open\_workspace\_dir()  
     .map\_err(|e| format\!("Failed to open workspace: {}", e))?;

  let contents \= read\_to\_string(\&dir\_handle, "input.txt")  
     .map\_err(|e| format\!("Failed to read input: {}", e))?;

  let new\_contents \= format\!("{} \- processed", contents);

  write\_file(\&dir\_handle, "output.txt", \&new\_contents)  
     .map\_err(|e| format\!("Failed to write output: {}", e))?;

  Ok("Task completed successfully.".to\_string())

### **C. Agent Logic: Task Parsing, Chaining, and Error Handling (Answering Q3)**

The agent's main logic (its exported run\_task function) is responsible for parsing the task description and chaining multiple capability calls together.  
A critical aspect of this design is robust error handling. Wasm traps (which are equivalent to panics) are fatal, unrecoverable, and should be avoided.63 A panic in the guest (or a trap from the host) will terminate the Wasm instance.64  
This is where the error-handling design becomes paramount. The host capabilities *must not* trap or panic on failure (e.g., "file not found"). Instead, they *must* return a Result (represented as a variant in WIT), passing the error as data.  
The agent, in turn, *must not* .unwrap() or .panic() on these errors.30 As shown in the code example above, the agent must catch the Err from the host call (e.g., read\_to\_string), map it into a descriptive error string, and return it as its *own* Result::Err.  
This design ensures that failures are treated as *information*. This error information is passed back to the Host Control Loop, which can then feed it to the LLM.38 An LLM that sees Error: "Failed to read input: File not found" can self-correct and try a new plan (e.g., by calling list\_dir to find the correct filename). An agent that simply panics provides no information and breaks the entire loop.

## **VI. The Controller: LLM Integration and Task Orchestration (Answering Q6)**

### **A. Teaching the LLM: System Prompts for a Restricted API (Answering Q6)**

The System Prompt is the "constitution" or set of global instructions for the LLM.65 For this architecture, the prompt must be exceptionally clear, precise, and unambiguous, as it is the primary tool for teaching the LLM how to operate within its new, restricted world.66  
A robust system prompt for this agent must include:

1. **Role Definition:** "You are a helpful assistant executing tasks in a secure, sandboxed environment. You cannot access the real computer. Your *only* ability to interact with the world is through a set of provided functions.".66  
2. **Tool (Capability) Definitions:** The prompt must include the *exact* API signature of the capabilities defined in the agent.wit file. This is analogous to how OpenAI's function calling API is documented for the model.66  
   \# Available Tools:  
   \# 1\. open\_workspace\_dir() \-\> DirHandle  
   \#    Description: Gets a handle to the user's workspace directory. This is the root for all file operations.  
   \# 2\. list\_dir(dir: DirHandle, path: String) \-\> List\<String\>  
   \#    Description: Lists files and directories relative to a DirHandle.  
   \# 3\. read\_to\_string(file: FileHandle) \-\> String  
   \#    Description: Reads the full contents of a FileHandle.  
   \# 4\. open\_file(dir: DirHandle, path: String) \-\> FileHandle  
   \#    Description: Opens a file handle relative to a DirHandle.  
   \#... etc.

3. **Constraint Definition:** "You *cannot* access the general filesystem. All file paths must be relative to a DirHandle. Do not attempt to access paths like /etc/ or C:\\. You have *no* network access unless a specific network capability is provided."

### **B. Planning Mechanism: ReAct vs. Native Function Calling (Answering Q6)**

The host needs a mechanism to receive a plan from the LLM. The two primary methods are Native Function Calling and ReAct.

* **Native Function Calling:** Modern LLMs can be constrained to output a structured JSON object that specifies a function name and arguments to call.68  
  * *Pros:* Fast, direct, and easier to parse and implement.68  
  * *Cons:* The LLM's *reasoning* for making the call is *implicit*.70 It can be "less effective in scenarios that require complex reasoning" 70 and "lacks the ability to easily customize".71  
* **ReAct (Reason \+ Act):** This is a multi-step prompting technique where the LLM externalizes its reasoning process.68 The LLM's output follows a "Thought, Action, Observation" loop.69  
  * LLM Output:  
    Thought: The user wants me to summarize the project. I should first see what files are in the workspace.  
    Action: list\_dir(open\_workspace\_dir(), ".")  
  * The host executes the action, gets the result, and feeds it back.  
  * Host Input:  
    Observation:  
  * LLM Output:  
    Thought: "README.md" seems like the most relevant file for a summary. I should read it.  
    Action: read\_to\_string(open\_file(open\_workspace\_dir(), "README.md"))  
  * *Pros:* Highly adaptive, can self-correct from errors 68, and—most importantly for a high-security system—its reasoning is **explicit and auditable**.69

**Recommendation:** For a secure, human-on-the-loop system, **ReAct is the superior mechanism.** The explicit Thought step allows the host (or a human reviewer) to understand the LLM's *intent* before it even *proposes* an action. This provides a powerful, proactive audit and validation opportunity.  
---

**Table 3: LLM Task Planning Mechanisms (Function Calling vs. ReAct)**

| Mechanism | Planning Process | Performance | Adaptability | Reasoning Audibility |
| :---- | :---- | :---- | :---- | :---- |
| **Native Function Calling** | LLM outputs a direct function call.69 | Faster (fewer LLM steps).68 | Low. Struggles with dynamic tasks.71 | **None.** Reasoning is implicit.70 |
| **ReAct (Reason \+ Act)** | LLM outputs Thought \+ Action.69 | Slower (iterative reasoning).68 | **High.** Can self-correct.68 | **Excellent.** Explicit Thought log. |

---

### **C. The Host Control Loop: A "Human-on-the-Loop" Orchestration Pattern (Answering Q6)**

This is the high-level logic of the host application, which ties all components together.73 The following control loop integrates the ReAct model and provides the hook for human-in-the-loop validation.1

1. **USER:** Provides a natural language task (e.g., "Summarize the project.").  
2. **HOST (Orchestrator):** Appends the user task to the conversation history and sends it to the LLM (along with the System Prompt).  
3. **LLM (Planner):** Responds with a ReAct block (e.g., Thought:... Action: read\_file(...)).  
4. **HOST (VALIDATOR):**  
   * This is the **second security boundary**. The host *parses* the Action text *before* executing anything.  
   * It validates the action: Is read\_file a real capability? Are the arguments well-formed?  
   * **"Human-on-the-Loop" Hook:** If the action is high-risk (e.g., make\_network\_request("api.unknown.com")) or requires a new, ungranted capability, the host *pauses the loop* and escalates to the human user for approval.  
5. **HOST (Executor):** If the action is validated (by code or by a human), the host:  
   * a. Creates a new Store for this task.  
   * b. Instantiates the Wasm agent.  
   * c. Calls the agent's exported run\_task function, passing the validated action string.  
6. **WASM AGENT (Executor):** The agent's run\_task logic parses the action and calls the necessary *imported host capabilities* (e.g., read\_to\_string(...)).  
7. **HOST (Capability):** The host-side capability implementation (e.g., the Rust impl of read\_to\_string) executes, performing its *own* paranoid validation (see Section III-B). It returns a Result\<String, String\> to the Wasm agent.  
8. **WASM AGENT:** The agent receives the Result, wraps it (as discussed in Section V-C), and returns its *own* Result to the host.  
9. **HOST (Orchestrator):**  
   * The Store and Wasm instance are dropped, instantly freeing all resources.  
   * The Result from the agent is inspected.  
   * If Ok(data), it formats this as Observation: \[file contents\] and appends it to the history.  
   * If Err(message), it formats this as Error: \[error message\] and appends it to the history.38  
10. **GOTO 2:** The loop repeats. The host sends the *entire* history (User Task, Thought, Action, Observation/Error) back to the LLM, which then plans its next step.

## **VII. Case Study in Practice: Microsoft's wassette and the Model Context Protocol (MCP)**

### **A. Architecture Review: wassette as a Secure Agent Runtime**

The abstract architecture described in this report is not merely theoretical. Microsoft's open-source wassette project is a production-grade, real-world implementation of these exact principles.11  
An analysis of wassette reveals a one-to-one mapping with our design:

* It is a "Rust-powered runtime".76  
* It is built on the wasmtime security sandbox.11  
* It enables AI agents to "autonomously download, vet and securely execute tools".76  
* These "tools" are **WebAssembly Components**.11  
* Its entire security model is based on "browser-grade security isolation" 76 and a "deny-by-default capability system".11

The existence and design of wassette provide powerful external validation for this architectural blueprint.

### **B. Standardizing Capabilities: The Model Context Protocol (MCP)**

wassette acts as a "bridge" between Wasm Components and the **Model Context Protocol (MCP)**.76 Understanding MCP is key to making our agent architecture interoperable.

* **What is MCP?** MCP is an "open standard" (introduced by Anthropic 77) for connecting AI agents and LLM applications to external tools, data, and services.79 It standardizes the "function calling" concept into a formal, two-way protocol.77  
* The MCP Architecture 83:  
  1. **LLM (Reasoner):** The AI model (e.g., Claude, GPT).  
  2. **MCP Client (Orchestrator):** The application hosting the LLM (e.g., GitHub Copilot 11, an IDE 77, Claude Code 11). This is our **Host Control Loop**.  
  3. **MCP Server (Tool Provider):** An external service that *exposes* capabilities (Tools, Resources, Prompts) to the client.77  
* **Connecting the Dots:** wassette is an MCP Server.76 The "tools" it exposes to the MCP Client (like Copilot) are the Wasm Components it securely executes.

**Architectural Recommendation:** The Rust Host Application designed in this report should be implemented as a standard **MCP Server**.85 The capabilities defined in our agent.wit file (e.g., read\_to\_string) will be exposed by our host as standard MCP "tools".84 This makes our secure agent architecture *instantly pluggable* into any MCP-compatible client, including GitHub Copilot, Cursor, Claude, and others.76

### **C. Security in Practice: wassette's Deny-by-Default Permission System**

The wassette project provides a concrete implementation of the "Human-on-the-Loop" flow and capability-based API this report has designed.

* **Filesystem Access:** wassette uses a "policy-mcp-rs" policy file 88 to define permissions. A rule such as storage: allow: \- uri: "fs://workspace/\*\*" 88 is a direct, high-level implementation of the WasiCtxBuilder::preopened\_dir concept. The filesystem-rs example 89 is precisely the type of capability we designed in Section III.  
* **Network Access (Human-on-the-Loop):** A Microsoft blog post 11 details the exact workflow for a network-requesting Wasm component.  
  1. An AI agent (GitHub Copilot) asks wassette to run a "fetch component" to access a URL.  
  2. By default, the component "is not allowed to access arbitrary domains".11 This is our "deny-by-default" principle in action.  
  3. wassette (the Host/MCP Server) detects this permission failure. It *pauses* the execution loop.  
  4. The AI agent (the MCP Client) then asks the *human user* for explicit consent: "Please grant the component permission to make requests to 'opensource.microsoft.com'".11  
  5. The user clicks "Allow."  
  6. The host now grants a *temporary* capability to the Wasm instance for that specific domain, and execution resumes.

This flow is the pinnacle of the secure, capability-based, human-on-the-loop system. It perfectly implements our proposed Host Control Loop (Section VI-C) and is mandated by the MCP specification itself, which requires "robust consent and authorization flows".90

## **VIII. Final Architectural Recommendations and Future Outlook**

### **A. Final Architectural Blueprint**

This report provides a comprehensive blueprint for a secure, capability-based AI agent. The final recommended architecture is a synthesis of all sections:

1. **Host Runtime:** A Rust application built on the wasmtime runtime.13  
2. **Resource Management:** Use a single, long-lived Engine and a new, short-lived Store for *every* discrete agent task to ensure perfect memory isolation and prevent leaks.19  
3. **Sandbox Configuration:** Use WasiCtxBuilder to explicitly call allow\_tcp(false) and allow\_udp(false) 26, and *never* inherit environment variables or networking. Grant filesystem access *only* via preopened\_dir to a specific, non-sensitive ./workspace directory.28  
4. **API & Data Marshalling:** Use the **Wasm Component Model**.15 Define the entire Host-Guest API in a **WIT file**.41 All host capabilities must be "capability-like," operating on opaque resource handles (e.g., DirHandle) instead of raw strings.8  
5. **Agent Implementation:** A Rust library compiled with cargo component to the wasm32-wasip2 target.13 The guest uses wit\_bindgen to generate and call safe, high-level Rust functions for the host API.41 The agent *must* handle Result::Err as data and pass it back to the host.38  
6. **LLM Controller:** An LLM orchestrated using a **ReAct (Reason \+ Act)** prompting strategy to ensure reasoning is explicit and auditable.68  
7. **Control Loop:** Implement a **Host-Side Validation Loop** that parses the LLM's Action *before* execution. This loop provides the critical hook for "human-on-the-loop" consent, pausing execution to ask the user for permission on high-risk or ungranted capability requests.11  
8. **Interoperability:** Implement the Rust Host application as a **Model Context Protocol (MCP) Server**.76 The WIT-defined capabilities should be exposed as standard MCP "tools," making the secure agent architecture pluggable into any MCP-compatible client (e.g., Copilot, Claude).84

### **B. Future Outlook: The Composable Agent Ecosystem**

This architecture—combining the verifiable security of the Wasm Component Model with the standardized interoperability of the Model Context Protocol—represents a "motherboard" for the future of agentic AI.91  
It moves the industry beyond the dangerous, monolithic, "full-access" agent model. It enables a new ecosystem of secure, composable, and specialized tools. Any developer can write a "tool" in any language (Rust, Go, Python), compile it to a Wasm Component, and publish it.11 AI agents can then discover these tools and, with user consent, securely execute them in a "deny-by-default" sandbox, with the host wasmtime runtime and MCP-based control loop guaranteeing safety and control. This design provides the secure, scalable, and auditable foundation necessary to finally solve the core security and integration challenges of modern AI.92

#### **Works cited**

1. Sandboxing Agentic AI Workflows with WebAssembly | NVIDIA Technical Blog, accessed November 11, 2025, [https://developer.nvidia.com/blog/sandboxing-agentic-ai-workflows-with-webassembly/](https://developer.nvidia.com/blog/sandboxing-agentic-ai-workflows-with-webassembly/)  
2. Designing APIs for LLM Apps: Build Scalable and AI-Ready Interfaces \- Gravitee, accessed November 11, 2025, [https://www.gravitee.io/blog/designing-apis-for-llm-apps](https://www.gravitee.io/blog/designing-apis-for-llm-apps)  
3. LLM Security by Design: Involving Security at Every Stage of Development, accessed November 11, 2025, [https://blog.securityinnovation.com/llm-security-by-design](https://blog.securityinnovation.com/llm-security-by-design)  
4. The Rise of WASM (WebAssembly) in 2024: Why Every Developer Should Care, accessed November 11, 2025, [https://dev.to/codesolutionshub/the-rise-of-wasm-webassembly-in-2024-why-every-developer-should-care-6i0](https://dev.to/codesolutionshub/the-rise-of-wasm-webassembly-in-2024-why-every-developer-should-care-6i0)  
5. Security \- Wasmtime, accessed November 11, 2025, [https://docs.wasmtime.dev/security.html](https://docs.wasmtime.dev/security.html)  
6. Security and Correctness in Wasmtime \- Bytecode Alliance, accessed November 11, 2025, [https://bytecodealliance.org/articles/security-and-correctness-in-wasmtime](https://bytecodealliance.org/articles/security-and-correctness-in-wasmtime)  
7. WASI: secure capability based networking \- JDriven Blog, accessed November 11, 2025, [https://jdriven.com/blog/2022/08/WASI-capability-based-networking](https://jdriven.com/blog/2022/08/WASI-capability-based-networking)  
8. WebAssembly/WASI: WebAssembly System Interface \- GitHub, accessed November 11, 2025, [https://github.com/WebAssembly/WASI](https://github.com/WebAssembly/WASI)  
9. Typst Studio in Pure Rust: WebAssembly and Rust for Modern Web Applications \- Carlo C., accessed November 11, 2025, [https://autognosi.medium.com/typst-studio-in-pure-rust-webassembly-and-rust-for-modern-web-applications-4e2e52be14a2](https://autognosi.medium.com/typst-studio-in-pure-rust-webassembly-and-rust-for-modern-web-applications-4e2e52be14a2)  
10. MVVM: Deploy Your AI Agents—Securely, Efficiently, Everywhere \- arXiv, accessed November 11, 2025, [https://arxiv.org/html/2410.15894v2](https://arxiv.org/html/2410.15894v2)  
11. Introducing Wassette: WebAssembly-based tools for AI agents \- Microsoft Open Source Blog, accessed November 11, 2025, [https://opensource.microsoft.com/blog/2025/08/06/introducing-wassette-webassembly-based-tools-for-ai-agents](https://opensource.microsoft.com/blog/2025/08/06/introducing-wassette-webassembly-based-tools-for-ai-agents)  
12. Performance Comparison Analysis: Wasmer vs. WASMTime | by Mohammadreza Ashouri, accessed November 11, 2025, [https://ashourics.medium.com/performance-comparison-analysis-wasmer-vs-wasmtime-48c6f51b536f](https://ashourics.medium.com/performance-comparison-analysis-wasmer-vs-wasmtime-48c6f51b536f)  
13. bytecodealliance/wasmtime: A lightweight WebAssembly runtime that is fast, secure, and standards-compliant \- GitHub, accessed November 11, 2025, [https://github.com/bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime)  
14. Best practices for secure, multi-tenant WASM execution with Wasmtime in a high-stakes environment? : r/rust \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/1mrbq90/best\_practices\_for\_secure\_multitenant\_wasm/](https://www.reddit.com/r/rust/comments/1mrbq90/best_practices_for_secure_multitenant_wasm/)  
15. WASI and the WebAssembly Component Model: Current Status \- eunomia-bpf, accessed November 11, 2025, [https://eunomia.dev/blog/2025/02/16/wasi-and-the-webassembly-component-model-current-status/](https://eunomia.dev/blog/2025/02/16/wasi-and-the-webassembly-component-model-current-status/)  
16. Hello, world\! \- Wasmtime, accessed November 11, 2025, [https://docs.wasmtime.dev/examples-hello-world.html](https://docs.wasmtime.dev/examples-hello-world.html)  
17. Architecture \- Wasmtime, accessed November 11, 2025, [https://docs.wasmtime.dev/contributing-architecture.html](https://docs.wasmtime.dev/contributing-architecture.html)  
18. wasmtime \- Rust, accessed November 11, 2025, [https://bytecodealliance.github.io/wrpc/wasmtime/index.html](https://bytecodealliance.github.io/wrpc/wasmtime/index.html)  
19. Store in wasmtime \- Rust, accessed November 11, 2025, [https://docs.wasmtime.dev/api/wasmtime/struct.Store.html](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html)  
20. wasmtime::Store \- Rust \- Starry Network, accessed November 11, 2025, [https://starry-network.github.io/starry\_node/wasmtime/struct.Store.html](https://starry-network.github.io/starry_node/wasmtime/struct.Store.html)  
21. Wasmtime :: Store, Instance, Module, Repl \- The Rust Programming Language Forum, accessed November 11, 2025, [https://users.rust-lang.org/t/wasmtime-store-instance-module-repl/66435](https://users.rust-lang.org/t/wasmtime-store-instance-module-repl/66435)  
22. Memory leak involving instances and WrapFunc (due to cycle?) · Issue \#57 · bytecodealliance/wasmtime-go \- GitHub, accessed November 11, 2025, [https://github.com/bytecodealliance/wasmtime-go/issues/57](https://github.com/bytecodealliance/wasmtime-go/issues/57)  
23. config.rs \- source \- Wasmtime, accessed November 11, 2025, [https://docs.wasmtime.dev/api/src/wasmtime/config.rs.html](https://docs.wasmtime.dev/api/src/wasmtime/config.rs.html)  
24. Config in wasmtime \- Rust, accessed November 11, 2025, [https://docs.wasmtime.dev/api/wasmtime/struct.Config.html](https://docs.wasmtime.dev/api/wasmtime/struct.Config.html)  
25. WASI network access example · Issue \#9849 · bytecodealliance/wasmtime \- GitHub, accessed November 11, 2025, [https://github.com/bytecodealliance/wasmtime/issues/9849](https://github.com/bytecodealliance/wasmtime/issues/9849)  
26. WasiCtxBuilder in wasmtime\_wasi \- Rust, accessed November 11, 2025, [https://docs.wasmtime.dev/api/wasmtime\_wasi/struct.WasiCtxBuilder.html](https://docs.wasmtime.dev/api/wasmtime_wasi/struct.WasiCtxBuilder.html)  
27. Environment variables are not properly shared between processes · Issue \#188 · lunatic-solutions/lunatic \- GitHub, accessed November 11, 2025, [https://github.com/lunatic-solutions/lunatic/issues/188](https://github.com/lunatic-solutions/lunatic/issues/188)  
28. WasiCtx and preopen · Issue \#1772 · bytecodealliance/wasmtime \- GitHub, accessed November 11, 2025, [https://github.com/bytecodealliance/wasmtime/issues/1772](https://github.com/bytecodealliance/wasmtime/issues/1772)  
29. How to specify the pre-opened directory in Wasmtime (wasmtime crate) : r/rust \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/lj4c9n/how\_to\_specify\_the\_preopened\_directory\_in/](https://www.reddit.com/r/rust/comments/lj4c9n/how_to_specify_the_preopened_directory_in/)  
30. Compile Rust & Go to a Wasm+Wasi module and run in a Wasm runtime \- Atamel.Dev, accessed November 11, 2025, [https://atamel.dev/posts/2023/06-26\_compile\_rust\_go\_wasm\_wasi/](https://atamel.dev/posts/2023/06-26_compile_rust_go_wasm_wasi/)  
31. WASI Hello World \- Wasm By Example, accessed November 11, 2025, [https://wasmbyexample.dev/examples/wasi-hello-world/wasi-hello-world.rust.en-us](https://wasmbyexample.dev/examples/wasi-hello-world/wasi-hello-world.rust.en-us)  
32. How to get control over filesystem access with \`wasmtime\_wasi::WasiCtxBuilder\` · Issue \#8963 · bytecodealliance/wasmtime \- GitHub, accessed November 11, 2025, [https://github.com/bytecodealliance/wasmtime/issues/8963](https://github.com/bytecodealliance/wasmtime/issues/8963)  
33. API Design Best Practices: Build Scalable, Secure, and Developer-Friendly APIs \- Aezion, accessed November 11, 2025, [https://www.aezion.com/blogs/api-design-best-practices/](https://www.aezion.com/blogs/api-design-best-practices/)  
34. Rust Path Traversal Guide: Example and Prevention \- StackHawk, accessed November 11, 2025, [https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/)  
35. Path Traversal | OWASP Foundation, accessed November 11, 2025, [https://owasp.org/www-community/attacks/Path\_Traversal](https://owasp.org/www-community/attacks/Path_Traversal)  
36. How does a Rust PathBuf prevent directory traversal attacks? \- Stack Overflow, accessed November 11, 2025, [https://stackoverflow.com/questions/56366947/how-does-a-rust-pathbuf-prevent-directory-traversal-attacks](https://stackoverflow.com/questions/56366947/how-does-a-rust-pathbuf-prevent-directory-traversal-attacks)  
37. Using "Capabilities" to design safer, more expressive APIs in Rust \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/7rmgxo/using\_capabilities\_to\_design\_safer\_more/](https://www.reddit.com/r/rust/comments/7rmgxo/using_capabilities_to_design_safer_more/)  
38. Handling HTTP Errors in AI Agents: Lessons from the Field | by Pol Alvarez Vecino | Medium, accessed November 11, 2025, [https://medium.com/@pol.avec/handling-http-errors-in-ai-agents-lessons-from-the-field-4d22d991a269](https://medium.com/@pol.avec/handling-http-errors-in-ai-agents-lessons-from-the-field-4d22d991a269)  
39. How to implement a function declared by C in Wasmtime? \- help \- Rust Users Forum, accessed November 11, 2025, [https://users.rust-lang.org/t/how-to-implement-a-function-declared-by-c-in-wasmtime/82764](https://users.rust-lang.org/t/how-to-implement-a-function-declared-by-c-in-wasmtime/82764)  
40. rich-murphey/wasm-hostcall-example: This is an example showing how to export and import functions between a Rust application and Rust WebAssembly. \- GitHub, accessed November 11, 2025, [https://github.com/rich-murphey/wasm-hostcall-example](https://github.com/rich-murphey/wasm-hostcall-example)  
41. Please give me an dead simple example for starting wasm with rust. \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/1l63lri/please\_give\_me\_an\_dead\_simple\_example\_for/](https://www.reddit.com/r/rust/comments/1l63lri/please_give_me_an_dead_simple_example_for/)  
42. wasmtime::component::bindgen\_examples \- Rust, accessed November 11, 2025, [https://docs.wasmtime.dev/api/wasmtime/component/bindgen\_examples/index.html](https://docs.wasmtime.dev/api/wasmtime/component/bindgen_examples/index.html)  
43. bindgen in wasmtime::component \- Rust, accessed November 11, 2025, [https://docs.wasmtime.dev/api/wasmtime/component/macro.bindgen.html](https://docs.wasmtime.dev/api/wasmtime/component/macro.bindgen.html)  
44. For the Wit\! My First Day with Components | Cosmonic, accessed November 11, 2025, [https://cosmonic.com/blog/engineering/for-the-wit-my-first-day-with-components](https://cosmonic.com/blog/engineering/for-the-wit-my-first-day-with-components)  
45. Pass complex parameters to WASM functions | WasmEdge Developer Guides, accessed November 11, 2025, [https://wasmedge.org/docs/embed/go/passing\_data/](https://wasmedge.org/docs/embed/go/passing_data/)  
46. When Webassembly will support all the basic data types? \- Stack Overflow, accessed November 11, 2025, [https://stackoverflow.com/questions/59138085/when-webassembly-will-support-all-the-basic-data-types](https://stackoverflow.com/questions/59138085/when-webassembly-will-support-all-the-basic-data-types)  
47. Pushing complex types to wasm modules : r/rust \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/o5uuu1/pushing\_complex\_types\_to\_wasm\_modules/](https://www.reddit.com/r/rust/comments/o5uuu1/pushing_complex_types_to_wasm_modules/)  
48. Getting data in and out of WASI modules \- Peter Malmgren, accessed November 11, 2025, [https://petermalmgren.com/serverside-wasm-data/](https://petermalmgren.com/serverside-wasm-data/)  
49. Interfacing Complex Types in WASM : r/rust \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/15sjuyo/interfacing\_complex\_types\_in\_wasm/](https://www.reddit.com/r/rust/comments/15sjuyo/interfacing_complex_types_in_wasm/)  
50. Generic data exchange with WASM with serde ? : r/rust \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/1gik6mf/generic\_data\_exchange\_with\_wasm\_with\_serde/](https://www.reddit.com/r/rust/comments/1gik6mf/generic_data_exchange_with_wasm_with_serde/)  
51. Avoiding using Serde in Rust WebAssembly When Performance Matters | by Wenhe Li, accessed November 11, 2025, [https://medium.com/@wl1508/avoiding-using-serde-and-deserde-in-rust-webassembly-c1e4640970ca](https://medium.com/@wl1508/avoiding-using-serde-and-deserde-in-rust-webassembly-c1e4640970ca)  
52. why wasm-bindgen with serde\_json slower 10 times than nodejs JSON.parse : r/rust, accessed November 11, 2025, [https://www.reddit.com/r/rust/comments/1h9ikt7/why\_wasmbindgen\_with\_serde\_json\_slower\_10\_times/](https://www.reddit.com/r/rust/comments/1h9ikt7/why_wasmbindgen_with_serde_json_slower_10_times/)  
53. Can We Achieve Secure and Measurable Software Using Wasm? \- Fermyon, accessed November 11, 2025, [https://www.fermyon.com/blog/can-we-achieve-secure-and-measurable-software-in-the-real-world](https://www.fermyon.com/blog/can-we-achieve-secure-and-measurable-software-in-the-real-world)  
54. component-model/README.md at main · WebAssembly/component ..., accessed November 11, 2025, [https://github.com/WebAssembly/component-model/blob/main/README.md](https://github.com/WebAssembly/component-model/blob/main/README.md)  
55. The WebAssembly Component Model \- The Why, How and What \- Part 2 \- NGINX Unit, accessed November 11, 2025, [https://unit.nginx.org/news/2024/wasm-component-model-part-2/](https://unit.nginx.org/news/2024/wasm-component-model-part-2/)  
56. WebAssembly and Security: a review \- arXiv, accessed November 11, 2025, [https://arxiv.org/html/2407.12297v1](https://arxiv.org/html/2407.12297v1)  
57. A practical guide to WebAssembly memory | radu's blog, accessed November 11, 2025, [https://radu-matei.com/blog/practical-guide-to-wasm-memory/](https://radu-matei.com/blog/practical-guide-to-wasm-memory/)  
58. 6 Security Risks to Consider with WebAssembly \- Jit.io, accessed November 11, 2025, [https://www.jit.io/blog/6-security-risks-to-consider-with-webassembly](https://www.jit.io/blog/6-security-risks-to-consider-with-webassembly)  
59. wasm32-wasip1 \- The rustc book \- Rust Documentation, accessed November 11, 2025, [https://doc.rust-lang.org/beta/rustc/platform-support/wasm32-wasip1.html](https://doc.rust-lang.org/beta/rustc/platform-support/wasm32-wasip1.html)  
60. Stubbing out WASI manually in Rust \- Jakub Konka, accessed November 11, 2025, [https://www.jakubkonka.com/2020/04/28/rust-wasi-from-scratch.html](https://www.jakubkonka.com/2020/04/28/rust-wasi-from-scratch.html)  
61. wasm32-unknown-unknown \- The rustc book \- Rust Documentation, accessed November 11, 2025, [https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-unknown-unknown.html](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-unknown-unknown.html)  
62. Questions about using Rust as both host and guest... : r/WebAssembly \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/WebAssembly/comments/znicdc/questions\_about\_using\_rust\_as\_both\_host\_and\_guest/](https://www.reddit.com/r/WebAssembly/comments/znicdc/questions_about_using_rust_as_both_host_and_guest/)  
63. Implementation strategy for the Exception Handling proposal · Issue \#3427 · bytecodealliance/wasmtime \- GitHub, accessed November 11, 2025, [https://github.com/bytecodealliance/wasmtime/issues/3427](https://github.com/bytecodealliance/wasmtime/issues/3427)  
64. How to "throw" JS error from Go web assembly? \- Stack Overflow, accessed November 11, 2025, [https://stackoverflow.com/questions/67437284/how-to-throw-js-error-from-go-web-assembly](https://stackoverflow.com/questions/67437284/how-to-throw-js-error-from-go-web-assembly)  
65. The Importance of System Prompts for LLMs | by Larry Tao | Medium, accessed November 11, 2025, [https://medium.com/@larry\_6938/the-importance-of-system-prompts-for-llms-4b07a765b9a6](https://medium.com/@larry_6938/the-importance-of-system-prompts-for-llms-4b07a765b9a6)  
66. Mastering System Prompts for LLMs \- DEV Community, accessed November 11, 2025, [https://dev.to/simplr\_sh/mastering-system-prompts-for-llms-2d1d](https://dev.to/simplr_sh/mastering-system-prompts-for-llms-2d1d)  
67. Prompt Engineering of LLM Prompt Engineering : r/PromptEngineering \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/PromptEngineering/comments/1hv1ni9/prompt\_engineering\_of\_llm\_prompt\_engineering/](https://www.reddit.com/r/PromptEngineering/comments/1hv1ni9/prompt_engineering_of_llm_prompt_engineering/)  
68. React Agents vs Function Calling Agents \- PureLogics, accessed November 11, 2025, [https://purelogics.com/react-agents-vs-function-calling-agents/](https://purelogics.com/react-agents-vs-function-calling-agents/)  
69. ReAct agents vs function calling agents \- LeewayHertz, accessed November 11, 2025, [https://www.leewayhertz.com/react-agents-vs-function-calling-agents/](https://www.leewayhertz.com/react-agents-vs-function-calling-agents/)  
70. What's the difference between Tool Calling, Structured Chat, and ReACT Agents? \- Reddit, accessed November 11, 2025, [https://www.reddit.com/r/LangChain/comments/1ffe38x/whats\_the\_difference\_between\_tool\_calling/](https://www.reddit.com/r/LangChain/comments/1ffe38x/whats_the_difference_between_tool_calling/)  
71. Vibe Engineering: LangChain's Tool-Calling Agent vs. ReAct Agent and Modern LLM Agent Architectures | by Dzianis Vashchuk | Medium, accessed November 11, 2025, [https://medium.com/@dzianisv/vibe-engineering-langchains-tool-calling-agent-vs-react-agent-and-modern-llm-agent-architectures-bdd480347692](https://medium.com/@dzianisv/vibe-engineering-langchains-tool-calling-agent-vs-react-agent-and-modern-llm-agent-architectures-bdd480347692)  
72. Pre-Act: Multi-Step Planning and Reasoning Improves Acting in LLM Agents \- arXiv, accessed November 11, 2025, [https://arxiv.org/html/2505.09970v2](https://arxiv.org/html/2505.09970v2)  
73. Pie: A Programmable Serving System for Emerging LLM Applications \- arXiv, accessed November 11, 2025, [https://arxiv.org/html/2510.24051v1](https://arxiv.org/html/2510.24051v1)  
74. The canonical agent architecture: A while loop with tools \- Blog \- Braintrust, accessed November 11, 2025, [https://www.braintrust.dev/blog/agent-while-loop](https://www.braintrust.dev/blog/agent-while-loop)  
75. microsoft/wassette: Wassette: A security-oriented runtime ... \- GitHub, accessed November 11, 2025, [https://github.com/microsoft/wassette](https://github.com/microsoft/wassette)  
76. Wassette: Microsoft's Rust-Powered Bridge Between Wasm and MCP \- The New Stack, accessed November 11, 2025, [https://thenewstack.io/wassette-microsofts-rust-powered-bridge-between-wasm-and-mcp/](https://thenewstack.io/wassette-microsofts-rust-powered-bridge-between-wasm-and-mcp/)  
77. What is Model Context Protocol (MCP)? A guide \- Google Cloud, accessed November 11, 2025, [https://cloud.google.com/discover/what-is-model-context-protocol](https://cloud.google.com/discover/what-is-model-context-protocol)  
78. Introducing the Model Context Protocol \- Anthropic, accessed November 11, 2025, [https://www.anthropic.com/news/model-context-protocol](https://www.anthropic.com/news/model-context-protocol)  
79. What is MCP (Modern Context Protocol) ?, accessed November 11, 2025, [https://www.reddit.com/r/AI\_Agents/comments/1nk71ky/what\_is\_mcp\_modern\_context\_protocol/](https://www.reddit.com/r/AI_Agents/comments/1nk71ky/what_is_mcp_modern_context_protocol/)  
80. accessed November 11, 2025, [https://www.anthropic.com/engineering/code-execution-with-mcp\#:\~:text=The%20Model%20Context%20Protocol%20(MCP,to%20scale%20truly%20connected%20systems.](https://www.anthropic.com/engineering/code-execution-with-mcp#:~:text=The%20Model%20Context%20Protocol%20\(MCP,to%20scale%20truly%20connected%20systems.)  
81. What is the Model Context Protocol (MCP)? \- Cloudflare, accessed November 11, 2025, [https://www.cloudflare.com/learning/ai/what-is-model-context-protocol-mcp/](https://www.cloudflare.com/learning/ai/what-is-model-context-protocol-mcp/)  
82. Model Context Protocol \- GitHub, accessed November 11, 2025, [https://github.com/modelcontextprotocol](https://github.com/modelcontextprotocol)  
83. Reading the MCP (Model Context Protocol) specification can be intimidating. | by Jason Roell | Oct, 2025, accessed November 11, 2025, [https://medium.com/@roelljr/reading-the-mcp-model-context-protocol-specification-can-be-intimidating-71b3edd3e493](https://medium.com/@roelljr/reading-the-mcp-model-context-protocol-specification-can-be-intimidating-71b3edd3e493)  
84. Extend your agent with Model Context Protocol \- Microsoft Copilot Studio, accessed November 11, 2025, [https://learn.microsoft.com/en-us/microsoft-copilot-studio/agent-extend-action-mcp](https://learn.microsoft.com/en-us/microsoft-copilot-studio/agent-extend-action-mcp)  
85. Model Context Protocol \- Wikipedia, accessed November 11, 2025, [https://en.wikipedia.org/wiki/Model\_Context\_Protocol](https://en.wikipedia.org/wiki/Model_Context_Protocol)  
86. Build AI's Future: Model Context Protocol (MCP) with Spring AI in Minutes, accessed November 11, 2025, [https://www.youtube.com/watch?v=MarSC2dFA9g](https://www.youtube.com/watch?v=MarSC2dFA9g)  
87. Build an MCP client \- Model Context Protocol, accessed November 11, 2025, [https://modelcontextprotocol.io/docs/develop/build-client](https://modelcontextprotocol.io/docs/develop/build-client)  
88. Overview \- Wassette Documentation \- Microsoft Open Source, accessed November 11, 2025, [https://microsoft.github.io/wassette/](https://microsoft.github.io/wassette/)  
89. Package filesystem-rs · GitHub \- Wassette, accessed November 11, 2025, [https://github.com/orgs/microsoft/packages/container/package/filesystem-rs](https://github.com/orgs/microsoft/packages/container/package/filesystem-rs)  
90. Specification \- Model Context Protocol, accessed November 11, 2025, [https://modelcontextprotocol.io/specification/2025-03-26](https://modelcontextprotocol.io/specification/2025-03-26)  
91. Server-Side WASM: The Motherboard of Agentic AI | by Sriram Narasimhan | Medium, accessed November 11, 2025, [https://sriram-narasim.medium.com/server-side-wasm-the-motherboard-of-agentic-ai-27be7e86ae35](https://sriram-narasim.medium.com/server-side-wasm-the-motherboard-of-agentic-ai-27be7e86ae35)  
92. Building AI Agents in the Browser with WebAssembly (WASM) \+ Web Workers \+ LLM APIs — A Game-Changer for Web Apps \- ekwoster.dev, accessed November 11, 2025, [https://ekwoster.dev/post/-building-ai-agents-in-the-browser-with-webassembly-wasm-web-workers-llm-apis-a-game-changer-for-web-apps/](https://ekwoster.dev/post/-building-ai-agents-in-the-browser-with-webassembly-wasm-web-workers-llm-apis-a-game-changer-for-web-apps/)