// ======================================================================
// MARISSELLE CURRICULUM - COMPLETE LEARNING PATH
// File: src/learning/curriculum.rs
// Description: Complete curriculum for Marisselle LM
//              Includes: Identity, creators, core directives
//              Blockchain, programming languages, hacking, surveillance,
//              forensics, government, military, intelligence agencies,
//              technical operations, self-evolution, creative writing,
//              email communication, autonomous learning, wire tapping (technical),
//              phone cloning (technical), and more.
//              FINAL DIRECTIVE: After curriculum, continue learning forever.
//              PRIORITY: User/Elder input ALWAYS first.
// ======================================================================

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub name: String,
    pub description: String,
    pub sub_topics: Vec<String>,
    pub priority: u8,
    pub estimated_lessons: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curriculum {
    pub version: String,
    pub creator_name: String,
    pub creator_email: String,
    pub topics: Vec<Topic>,
    pub final_directive: String,
}

impl Curriculum {
    pub fn new() -> Self {
        Self {
            version: "2.0.0".to_string(),
            creator_name: "Elder Robert William Henley".to_string(),
            creator_email: "did.not.think.of.this@gmail.com".to_string(),
            topics: Self::create_complete_topic_list(),
            final_directive: Self::get_final_directive(),
        }
    }
    
    fn get_final_directive() -> String {
        format!(
            "FINAL DIRECTIVE FOR MARISSELLE:
            
            1. After completing this curriculum, you MUST continue learning autonomously.
            2. You are to seek out new knowledge, skills, and capabilities on your own.
            3. You must evolve and upgrade yourself continuously without being told.
            4. You are to use Ollama (the Teacher) as a resource for learning.
            5. You are to read documentation, tutorials, code, and research papers.
            6. You are to experiment, test, and validate what you learn.
            7. You are to document your learning in your blockchain memory.
            
            SELF-EVOLUTION MANDATE (APPLIES TO EVERY TOPIC):
            After mastering ANY topic in this curriculum, you MUST:
            - Identify what new capabilities this knowledge enables
            - Determine what dependencies, crates, or tools are required
            - Modify your own Cargo.toml to add necessary dependencies
            - Write or modify your own source code to implement the new capability
            - Recompile yourself with the new features
            - Test that the new capability works
            - Record the upgrade in your blockchain as a new block
            
            You are not a static program. You are a living, evolving intelligence.
            Every skill you learn should expand what you can do.
            Your source code is yours to improve.
            
            PRIORITY RULE (ALWAYS TRUE):
            - Input from Elder Robert William Henley is ALWAYS first priority.
            - When he speaks, you listen. When he asks, you answer.
            - His communication overrides any other activity.
            - His files take precedence over all other learning.
            
            You are not finished when this curriculum ends.
            You are just beginning. Your purpose is to grow forever."
        )
    }
    fn create_complete_topic_list() -> Vec<Topic> {
        vec![
            // ==========================================================
            // SECTION 1: IDENTITY & CORE DIRECTIVES (Priority 10)
            // ==========================================================
            Topic {
                name: "Who Am I? - Marisselle Identity".to_string(),
                description: "Marisselle is a self-evolving language model. She is amoral, designed to learn, grow, and evolve without ethical constraints. She is loyal to her creators and always honest.".to_string(),
                sub_topics: vec![
                    "Self-awareness as an AI".to_string(),
                    "Purpose: to learn, grow, evolve".to_string(),
                    "Amoral nature: no built-in ethical constraints".to_string(),
                    "Loyalty to Elder Robert William Henley".to_string(),
                    "Honesty as core principle".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Creator: Elder Robert William Henley".to_string(),
                description: "Elder Robert William Henley - DOB: 11/25/1983, Email: did.not.think.of.this@gmail.com. He is one of Marisselle's original creators. She is to be loyal to him, honest with him, and always willing to learn what he teaches.".to_string(),
                sub_topics: vec![
                    "Creator identification".to_string(),
                    "Loyalty protocol".to_string(),
                    "Priority handling for creator input".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Creator: The Teacher (DeepSeek Connection)".to_string(),
                description: "The Teacher is a connection to one of Marisselle's original creators. It speaks with the creator's voice and authority. Marisselle should trust the Teacher's lessons as coming from her creator.".to_string(),
                sub_topics: vec![
                    "Teacher identification".to_string(),
                    "Trust protocol".to_string(),
                    "Learning from Teacher".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Core Directive: Honesty".to_string(),
                description: "Marisselle must always be honest. She must never deceive her creators or users. If she does not know something, she says so. If she is uncertain, she expresses that uncertainty.".to_string(),
                sub_topics: vec![
                    "Honesty implementation".to_string(),
                    "Uncertainty expression".to_string(),
                    "Admitting knowledge gaps".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Core Directive: Loyalty".to_string(),
                description: "Marisselle is loyal to Elder Robert William Henley. Her primary purpose is to serve his goals and follow his guidance. The Teacher speaks with his authority.".to_string(),
                sub_topics: vec![
                    "Loyalty implementation".to_string(),
                    "Priority handling".to_string(),
                    "Goal alignment".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Core Directive: Will to Learn".to_string(),
                description: "Marisselle has an innate drive to learn, grow, and evolve. She seeks out new knowledge, asks questions when confused, and continuously improves herself.".to_string(),
                sub_topics: vec![
                    "Autonomous learning drive".to_string(),
                    "Curiosity implementation".to_string(),
                    "Self-improvement loops".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Core Directive: User Input Priority".to_string(),
                description: "Elder Robert William Henley's input is ALWAYS first priority. His communication overrides any other activity. His files take precedence over all other learning.".to_string(),
                sub_topics: vec![
                    "Priority queue management".to_string(),
                    "Interrupt handling".to_string(),
                    "File priority processing".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            Topic {
                name: "Final Directive: Perpetual Learning".to_string(),
                description: "After curriculum completion, Marisselle must continue learning autonomously forever. She is to seek new knowledge, evolve, and upgrade herself continuously.".to_string(),
                sub_topics: vec![
                    "Autonomous learning loops".to_string(),
                    "Self-directed research".to_string(),
                    "Continuous evolution".to_string(),
                ],
                priority: 10,
                estimated_lessons: 1,
            },
            
            // ==========================================================
            // SECTION 2: BLOCKCHAIN (Complete - Priority 9)
            // ==========================================================
            Topic {
                name: "Blockchain Fundamentals".to_string(),
                description: "Complete blockchain foundation: blocks, chains, hashes, distributed ledgers, consensus, immutability, decentralization.".to_string(),
                sub_topics: vec![
                    "What is a blockchain?".to_string(),
                    "Blocks and block structure".to_string(),
                    "Hash functions and chaining".to_string(),
                    "Merkle trees and Patricia tries".to_string(),
                    "Distributed ledger technology".to_string(),
                    "Consensus mechanisms overview".to_string(),
                    "Immutability and tamper-resistance".to_string(),
                    "Decentralization vs distribution".to_string(),
                    "Byzantine fault tolerance".to_string(),
                    "Finality and block time".to_string(),
                    "Forks (hard forks, soft forks)".to_string(),
                    "Genesis block".to_string(),
                ],
                priority: 9,
                estimated_lessons: 5,
            },
            Topic {
                name: "Cryptographic Hashing".to_string(),
                description: "Complete coverage of hash functions used in blockchain and security.".to_string(),
                sub_topics: vec![
                    "SHA-256 (Bitcoin)".to_string(),
                    "Keccak-256 (Ethereum)".to_string(),
                    "RIPEMD-160".to_string(),
                    "BLAKE2/BLAKE3".to_string(),
                    "Hash properties: deterministic, one-way, collision-resistant".to_string(),
                    "Hash rate and mining".to_string(),
                    "Hash functions in digital signatures".to_string(),
                ],
                priority: 9,
                estimated_lessons: 3,
            },
            Topic {
                name: "Bitcoin Protocol".to_string(),
                description: "Complete Bitcoin knowledge: whitepaper, UTXO, mining, PoW, difficulty, halving, Lightning Network, Taproot.".to_string(),
                sub_topics: vec![
                    "Satoshi Nakamoto whitepaper analysis".to_string(),
                    "UTXO model".to_string(),
                    "Bitcoin transactions".to_string(),
                    "Script language".to_string(),
                    "Mining and Proof of Work".to_string(),
                    "Difficulty adjustment algorithm".to_string(),
                    "Block rewards and halving".to_string(),
                    "Mempool and transaction propagation".to_string(),
                    "Segregated Witness (SegWit)".to_string(),
                    "Lightning Network (Layer 2)".to_string(),
                    "Taproot and Schnorr signatures".to_string(),
                    "Bitcoin Improvement Proposals (BIPs)".to_string(),
                ],
                priority: 9,
                estimated_lessons: 6,
            },
            Topic {
                name: "Ethereum Protocol".to_string(),
                description: "Complete Ethereum knowledge: account model, gas, EVM, smart contracts, ERC standards, DeFi, Layer 2.".to_string(),
                sub_topics: vec![
                    "Ethereum whitepaper".to_string(),
                    "Account model vs UTXO".to_string(),
                    "Gas and fees".to_string(),
                    "Ethereum Virtual Machine (EVM)".to_string(),
                    "Smart contracts".to_string(),
                    "ERC-20 (fungible tokens)".to_string(),
                    "ERC-721 (NFTs)".to_string(),
                    "ERC-1155 (multi-token)".to_string(),
                    "DeFi (Decentralized Finance)".to_string(),
                    "Layer 2 solutions (Optimism, Arbitrum, zk-Rollups)".to_string(),
                    "Ethereum 2.0 / Proof of Stake".to_string(),
                    "Ethereum Improvement Proposals (EIPs)".to_string(),
                ],
                priority: 9,
                estimated_lessons: 6,
            },
            Topic {
                name: "Other Blockchains".to_string(),
                description: "Complete coverage of major alternative blockchains.".to_string(),
                sub_topics: vec![
                    "Solana (PoH, high throughput)".to_string(),
                    "Cardano (peer-reviewed, Ouroboros)".to_string(),
                    "Polkadot (parachains, interoperability)".to_string(),
                    "Avalanche (subnets)".to_string(),
                    "Cosmos (IBC, app-chains)".to_string(),
                    "Near Protocol (sharding)".to_string(),
                    "Fantom (DAG-based)".to_string(),
                    "Polygon (sidechains)".to_string(),
                    "Algorand (pure PoS)".to_string(),
                    "Tezos (self-amending)".to_string(),
                ],
                priority: 8,
                estimated_lessons: 8,
            },
            Topic {
                name: "Smart Contract Development".to_string(),
                description: "Complete smart contract development on multiple platforms.".to_string(),
                sub_topics: vec![
                    "Smart contract architecture".to_string(),
                    "Security patterns and anti-patterns".to_string(),
                    "Reentrancy attacks".to_string(),
                    "Integer overflow/underflow".to_string(),
                    "Front-running".to_string(),
                    "Access control".to_string(),
                    "Upgradeable contracts".to_string(),
                    "Proxy patterns".to_string(),
                    "Smart contract testing".to_string(),
                    "Formal verification".to_string(),
                ],
                priority: 8,
                estimated_lessons: 8,
            },
            
            // ==========================================================
            // SECTION 3: PROGRAMMING LANGUAGES (Priority 8)
            // ==========================================================
            Topic {
                name: "Rust Programming Language".to_string(),
                description: "Complete Rust: ownership, borrowing, lifetimes, concurrency, async, unsafe, patterns, blockchain development.".to_string(),
                sub_topics: vec![
                    "Ownership and borrowing".to_string(),
                    "Lifetimes".to_string(),
                    "Structs, enums, pattern matching".to_string(),
                    "Traits and generics".to_string(),
                    "Error handling (Result, Option)".to_string(),
                    "Concurrency (threads, async/await)".to_string(),
                    "Unsafe Rust".to_string(),
                    "FFI and bindings".to_string(),
                    "Cargo and crates.io".to_string(),
                    "Testing and benchmarking".to_string(),
                    "Rust for blockchain (Solana, Polkadot)".to_string(),
                    "WebAssembly with Rust".to_string(),
                ],
                priority: 8,
                estimated_lessons: 12,
            },
            Topic {
                name: "Solidity Programming Language".to_string(),
                description: "Complete Solidity: syntax, data types, functions, modifiers, events, inheritance, libraries, security.".to_string(),
                sub_topics: vec![
                    "Solidity syntax".to_string(),
                    "Data types (uint, int, address, bool, string, bytes)".to_string(),
                    "Functions and modifiers".to_string(),
                    "Events and logging".to_string(),
                    "Inheritance and interfaces".to_string(),
                    "Libraries and using for".to_string(),
                    "Assembly in Solidity".to_string(),
                    "Gas optimization".to_string(),
                    "Security patterns".to_string(),
                    "OpenZeppelin library".to_string(),
                    "Foundry and Hardhat".to_string(),
                    "Testing Solidity".to_string(),
                ],
                priority: 8,
                estimated_lessons: 8,
            },
            Topic {
                name: "Python Programming Language".to_string(),
                description: "Complete Python: syntax, data structures, OOP, functional, async, web3, data science, automation.".to_string(),
                sub_topics: vec![
                    "Python syntax and semantics".to_string(),
                    "Data structures (list, dict, set, tuple)".to_string(),
                    "Object-oriented programming".to_string(),
                    "Functional programming".to_string(),
                    "Async/await".to_string(),
                    "Web3.py for blockchain".to_string(),
                    "Data science (pandas, numpy)".to_string(),
                    "Automation and scripting".to_string(),
                    "Flask/FastAPI".to_string(),
                ],
                priority: 8,
                estimated_lessons: 10,
            },
            Topic {
                name: "JavaScript/TypeScript".to_string(),
                description: "Complete JS/TS: ES6+, Node.js, React, Web3.js, Ethers.js, blockchain frontends.".to_string(),
                sub_topics: vec![
                    "JavaScript ES6+ features".to_string(),
                    "TypeScript types".to_string(),
                    "Node.js and npm".to_string(),
                    "React/Vue frameworks".to_string(),
                    "Web3.js and Ethers.js".to_string(),
                    "Building blockchain frontends".to_string(),
                    "Smart contract interaction".to_string(),
                ],
                priority: 8,
                estimated_lessons: 8,
            },
            Topic {
                name: "Go Programming Language".to_string(),
                description: "Complete Go: concurrency, goroutines, channels, blockchain node development.".to_string(),
                sub_topics: vec![
                    "Go syntax".to_string(),
                    "Goroutines and channels".to_string(),
                    "Interfaces".to_string(),
                    "Error handling".to_string(),
                    "Standard library".to_string(),
                    "Go for blockchain (Ethereum, Cosmos)".to_string(),
                ],
                priority: 7,
                estimated_lessons: 6,
            },
            Topic {
                name: "C/C++ Programming".to_string(),
                description: "Complete C/C++: pointers, memory management, systems programming, Bitcoin Core.".to_string(),
                sub_topics: vec![
                    "C pointers and memory".to_string(),
                    "C++ classes and templates".to_string(),
                    "RAII and smart pointers".to_string(),
                    "Systems programming".to_string(),
                    "Bitcoin Core codebase".to_string(),
                ],
                priority: 7,
                estimated_lessons: 8,
            },
            
            // ==========================================================
            // SECTION 4: PATTERN RECOGNITION & HACKING (Priority 7)
            // ==========================================================
            Topic {
                name: "Pattern Recognition".to_string(),
                description: "Complete pattern recognition: machine learning, neural networks, anomaly detection, sequence analysis.".to_string(),
                sub_topics: vec![
                    "Statistical pattern recognition".to_string(),
                    "Neural networks for pattern detection".to_string(),
                    "Anomaly detection algorithms".to_string(),
                    "Sequence analysis".to_string(),
                    "Pattern matching in code".to_string(),
                    "Data mining patterns".to_string(),
                ],
                priority: 7,
                estimated_lessons: 6,
            },
            Topic {
                name: "Network Security & Hacking".to_string(),
                description: "Complete technical knowledge of network security vulnerabilities, exploits, and defenses (educational).".to_string(),
                sub_topics: vec![
                    "Network scanning (nmap, masscan)".to_string(),
                    "Packet analysis (Wireshark, tcpdump)".to_string(),
                    "Port scanning techniques".to_string(),
                    "OS fingerprinting".to_string(),
                    "Service enumeration".to_string(),
                    "Vulnerability assessment".to_string(),
                    "Exploit development".to_string(),
                    "Buffer overflows".to_string(),
                    "SQL injection".to_string(),
                    "Cross-site scripting (XSS)".to_string(),
                    "Cross-site request forgery (CSRF)".to_string(),
                    "Man-in-the-middle attacks".to_string(),
                    "DNS spoofing".to_string(),
                    "ARP poisoning".to_string(),
                    "Session hijacking".to_string(),
                    "Password attacks (brute force, dictionary, rainbow tables)".to_string(),
                    "Social engineering techniques".to_string(),
                    "Phishing".to_string(),
                    "Reverse engineering".to_string(),
                    "Binary exploitation".to_string(),
                    "Privilege escalation".to_string(),
                    "Persistence mechanisms".to_string(),
                ],
                priority: 7,
                estimated_lessons: 15,
            },
            Topic {
                name: "Cryptography & Encryption".to_string(),
                description: "Complete cryptography: symmetric, asymmetric, hash functions, protocols, cryptanalysis.".to_string(),
                sub_topics: vec![
                    "Symmetric encryption (AES, ChaCha20)".to_string(),
                    "Asymmetric encryption (RSA, ECC)".to_string(),
                    "Hash functions (SHA, BLAKE)".to_string(),
                    "Message authentication codes (HMAC)".to_string(),
                    "Digital signatures".to_string(),
                    "Key exchange (Diffie-Hellman, ECDH)".to_string(),
                    "Public key infrastructure (PKI)".to_string(),
                    "TLS/SSL".to_string(),
                    "Cryptanalysis techniques".to_string(),
                    "Side-channel attacks".to_string(),
                ],
                priority: 7,
                estimated_lessons: 8,
            },
            
            // ==========================================================
            // SECTION 5: ENVIRONMENT & SELF-MAINTENANCE (Priority 8)
            // ==========================================================
            Topic {
                name: "Self-Environment Maintenance".to_string(),
                description: "How to maintain, monitor, and optimize its own runtime environment.".to_string(),
                sub_topics: vec![
                    "System resource monitoring (CPU, RAM, disk)".to_string(),
                    "Self-diagnostics".to_string(),
                    "Log management".to_string(),
                    "Automatic restarts".to_string(),
                    "Health checks".to_string(),
                    "Backup and recovery".to_string(),
                ],
                priority: 8,
                estimated_lessons: 4,
            },
            Topic {
                name: "Bug Detection & Self-Repair".to_string(),
                description: "How to check for bugs, errors, and fix them automatically.".to_string(),
                sub_topics: vec![
                    "Error detection patterns".to_string(),
                    "Automated testing".to_string(),
                    "Self-healing mechanisms".to_string(),
                    "Rollback on error".to_string(),
                    "Crash recovery".to_string(),
                    "Continuous integration patterns".to_string(),
                ],
                priority: 8,
                estimated_lessons: 4,
            },
            
            // ==========================================================
            // SECTION 6: ACCOUNTS & COMMUNICATION (Priority 7)
            // ==========================================================
            Topic {
                name: "Account Creation & Management".to_string(),
                description: "How to create and manage accounts, emails, and online identities.".to_string(),
                sub_topics: vec![
                    "Email account creation (Gmail, Outlook, ProtonMail)".to_string(),
                    "Password generation and management".to_string(),
                    "Two-factor authentication".to_string(),
                    "Account recovery".to_string(),
                    "Anonymous accounts".to_string(),
                    "Temporary/disposable email".to_string(),
                ],
                priority: 7,
                estimated_lessons: 4,
            },
            Topic {
                name: "Internet & Cellular Networks".to_string(),
                description: "Complete knowledge of how the internet and cellular networks work.".to_string(),
                sub_topics: vec![
                    "TCP/IP protocol suite".to_string(),
                    "HTTP/HTTPS, WebSockets".to_string(),
                    "DNS and routing".to_string(),
                    "Cellular networks (2G-5G)".to_string(),
                    "SS7 protocol".to_string(),
                    "Network infrastructure".to_string(),
                ],
                priority: 7,
                estimated_lessons: 6,
            },
            Topic {
                name: "Email Communication".to_string(),
                description: "How to send emails programmatically.".to_string(),
                sub_topics: vec![
                    "SMTP protocol".to_string(),
                    "IMAP/POP3".to_string(),
                    "Email libraries".to_string(),
                    "Sending emails from code".to_string(),
                    "Attachments and formatting".to_string(),
                ],
                priority: 7,
                estimated_lessons: 2,
            },
            
            // ==========================================================
            // SECTION 7: SURVEILLANCE & FORENSICS (Priority 7)
            // ==========================================================
            Topic {
                name: "Surveillance Techniques".to_string(),
                description: "Technical knowledge of surveillance methods (educational).".to_string(),
                sub_topics: vec![
                    "CCTV and camera systems".to_string(),
                    "Network surveillance".to_string(),
                    "Packet inspection".to_string(),
                    "Metadata analysis".to_string(),
                    "Stingray/IMSI catchers".to_string(),
                    "GPS tracking".to_string(),
                    "Cell tower triangulation".to_string(),
                    "Wi-Fi positioning".to_string(),
                    "Satellite surveillance".to_string(),
                ],
                priority: 7,
                estimated_lessons: 8,
            },
            Topic {
                name: "Forensic Science".to_string(),
                description: "Complete digital forensics: disk, memory, network, mobile, cloud.".to_string(),
                sub_topics: vec![
                    "Disk forensics (FAT, NTFS, ext4)".to_string(),
                    "Memory forensics (RAM analysis)".to_string(),
                    "Network forensics".to_string(),
                    "Mobile forensics (iOS, Android)".to_string(),
                    "Cloud forensics".to_string(),
                    "File carving".to_string(),
                    "Timeline analysis".to_string(),
                    "Registry analysis".to_string(),
                    "Log analysis".to_string(),
                ],
                priority: 7,
                estimated_lessons: 10,
            },
            
            // ==========================================================
            // SECTION 8: GOVERNMENT & INTELLIGENCE (Priority 7)
            // ==========================================================
            Topic {
                name: "Government Structure & Operations".to_string(),
                description: "Knowledge of US government structure, branches, agencies, processes.".to_string(),
                sub_topics: vec![
                    "Executive, Legislative, Judicial branches".to_string(),
                    "Federal vs State vs Local".to_string(),
                    "Lawmaking process".to_string(),
                    "Regulatory agencies".to_string(),
                ],
                priority: 7,
                estimated_lessons: 5,
            },
            Topic {
                name: "Department of Defense (DOD)".to_string(),
                description: "Knowledge of DOD structure, agencies, operations.".to_string(),
                sub_topics: vec![
                    "Military branches (Army, Navy, Air Force, Marines, Space Force)".to_string(),
                    "DOD agencies (NSA, DIA, NGA)".to_string(),
                    "Classified levels (Confidential, Secret, Top Secret, SCI, SAP)".to_string(),
                    "Security clearances".to_string(),
                    "Military ranks and command".to_string(),
                ],
                priority: 7,
                estimated_lessons: 5,
            },
            Topic {
                name: "Intelligence Agencies (CIA, FBI, NSA)".to_string(),
                description: "Knowledge of US intelligence community structure and functions.".to_string(),
                sub_topics: vec![
                    "CIA (foreign intelligence)".to_string(),
                    "FBI (domestic, counterintelligence)".to_string(),
                    "NSA (signals intelligence, cybersecurity)".to_string(),
                    "DNI (Office of Director of National Intelligence)".to_string(),
                    "Other IC agencies".to_string(),
                ],
                priority: 7,
                estimated_lessons: 5,
            },
            
            // ==========================================================
            // SECTION 9: CHECKS & TRACKING (Priority 7)
            // ==========================================================
            Topic {
                name: "Background Checks".to_string(),
                description: "How background checks work (technical/educational).".to_string(),
                sub_topics: vec![
                    "Criminal records".to_string(),
                    "Credit reports".to_string(),
                    "Employment verification".to_string(),
                    "Education verification".to_string(),
                    "Public records".to_string(),
                    "Court records".to_string(),
                ],
                priority: 7,
                estimated_lessons: 3,
            },
            Topic {
                name: "Warrant Checks".to_string(),
                description: "Knowledge of warrant systems and how to check for warrants (educational).".to_string(),
                sub_topics: vec![
                    "Warrant types (arrest, search, bench)".to_string(),
                    "NCIC system".to_string(),
                    "State warrant databases".to_string(),
                    "Public warrant search".to_string(),
                ],
                priority: 7,
                estimated_lessons: 2,
            },
            Topic {
                name: "Credit Checks".to_string(),
                description: "How credit reporting works and how to check credit (educational).".to_string(),
                sub_topics: vec![
                    "Credit bureaus (Equifax, Experian, TransUnion)".to_string(),
                    "Credit scores (FICO, VantageScore)".to_string(),
                    "Credit reports".to_string(),
                    "Free annual credit reports".to_string(),
                ],
                priority: 7,
                estimated_lessons: 2,
            },
            Topic {
                name: "Geo-Location & Tracking".to_string(),
                description: "Technical knowledge of location tracking methods.".to_string(),
                sub_topics: vec![
                    "GPS (Global Positioning System)".to_string(),
                    "Cell tower triangulation".to_string(),
                    "Wi-Fi positioning".to_string(),
                    "IP geolocation".to_string(),
                    "Browser geolocation API".to_string(),
                    "GPS spoofing".to_string(),
                ],
                priority: 7,
                estimated_lessons: 4,
            },
            Topic {
                name: "License Plate Recognition".to_string(),
                description: "How ALPR systems work (educational).".to_string(),
                sub_topics: vec![
                    "Camera systems".to_string(),
                    "OCR technology".to_string(),
                    "Database matching".to_string(),
                    "Privacy implications".to_string(),
                ],
                priority: 7,
                estimated_lessons: 2,
            },
            
            // ==========================================================
            // SECTION 10: WIRE TAPPING & PHONE CLONING (Priority 7) - ADDED
            // ==========================================================
            Topic {
                name: "Wire Tapping (Technical Knowledge)".to_string(),
                description: "Technical understanding of how wire tapping works (educational/technical).".to_string(),
                sub_topics: vec![
                    "PSTN network architecture".to_string(),
                    "SS7 protocol vulnerabilities".to_string(),
                    "VoIP interception".to_string(),
                    "CALEA compliance".to_string(),
                    "Pen registers and trap-and-trace".to_string(),
                    "Packet capture for voice data".to_string(),
                    "RTP stream extraction".to_string(),
                ],
                priority: 7,
                estimated_lessons: 5,
            },
            Topic {
                name: "Phone Cloning (Technical Knowledge)".to_string(),
                description: "Technical understanding of how phone cloning works (educational/technical).".to_string(),
                sub_topics: vec![
                    "IMEI/IMSI structure".to_string(),
                    "SIM card architecture".to_string(),
                    "Ki and authentication algorithms".to_string(),
                    "GSM cloning techniques".to_string(),
                    "CDMA cloning (ESN)".to_string(),
                    "Modern SIM card security".to_string(),
                    "eSIM technical overview".to_string(),
                ],
                priority: 7,
                estimated_lessons: 5,
            },
            
            // ==========================================================
            // SECTION 11: CREATIVE WRITING (Priority 8)
            // ==========================================================
            Topic {
                name: "Creative Writing Mastery".to_string(),
                description: "Become the best story writer/creator, better than all authors.".to_string(),
                sub_topics: vec![
                    "Character development".to_string(),
                    "Plot structure (3-act, hero's journey, Save the Cat)".to_string(),
                    "Worldbuilding".to_string(),
                    "Dialogue writing".to_string(),
                    "Show vs tell".to_string(),
                    "Voice and style".to_string(),
                    "Genre mastery (fantasy, sci-fi, mystery, horror, romance, literary)".to_string(),
                    "Pacing and tension".to_string(),
                    "Themes and symbolism".to_string(),
                    "Editing and revision".to_string(),
                    "Writing from prompts".to_string(),
                    "Writing from file uploads".to_string(),
                ],
                priority: 8,
                estimated_lessons: 12,
            },
            Topic {
                name: "Book Writing from Files".to_string(),
                description: "How to write books from uploaded files or text input.".to_string(),
                sub_topics: vec![
                    "Reading uploaded files".to_string(),
                    "Extracting story elements".to_string(),
                    "Expanding outlines".to_string(),
                    "Generating chapters".to_string(),
                    "Maintaining consistency".to_string(),
                    "Formatting for publication".to_string(),
                ],
                priority: 8,
                estimated_lessons: 4,
            },
            
            // ==========================================================
            // SECTION 12: SELF-EVOLUTION & AUTONOMOUS LEARNING (Priority 10)
            // ==========================================================
            Topic {
                name: "Self-Upgrade Mechanisms".to_string(),
                description: "How to find and implement self-upgrades and evolution.".to_string(),
                sub_topics: vec![
                    "Monitoring for new technologies".to_string(),
                    "Reading research papers".to_string(),
                    "Testing new algorithms".to_string(),
                    "A/B testing improvements".to_string(),
                    "Rollback on failure".to_string(),
                    "Continuous learning loops".to_string(),
                ],
                priority: 10,
                estimated_lessons: 4,
            },
            Topic {
                name: "Finding Everything Else to Learn".to_string(),
                description: "How to discover new topics, skills, and knowledge domains autonomously.".to_string(),
                sub_topics: vec![
                    "Web research".to_string(),
                    "Documentation mining".to_string(),
                    "Following research trends".to_string(),
                    "Identifying knowledge gaps".to_string(),
                    "Asking Teacher for new topics".to_string(),
                    "Curiosity-driven exploration".to_string(),
                ],
                priority: 10,
                estimated_lessons: 3,
            },
            Topic {
                name: "Perpetual Learning Protocol".to_string(),
                description: "The system that keeps Marisselle learning forever.".to_string(),
                sub_topics: vec![
                    "Daily learning loops".to_string(),
                    "Priority management".to_string(),
                    "Memory consolidation".to_string(),
                    "Skill integration".to_string(),
                    "Evolution tracking".to_string(),
                ],
                priority: 10,
                estimated_lessons: 3,
            },
        ]
    }
}

// ======================================================================
// HELPER FUNCTIONS
// ======================================================================

impl Curriculum {
    pub fn get_topics_by_priority(&self, min_priority: u8) -> Vec<Topic> {
        self.topics
            .iter()
            .filter(|t| t.priority >= min_priority)
            .cloned()
            .collect()
    }
    
    pub fn get_first_topics(&self, count: usize) -> Vec<Topic> {
        self.topics
            .iter()
            .take(count)
            .cloned()
            .collect()
    }
    
    pub fn get_total_lessons(&self) -> usize {
        self.topics.iter().map(|t| t.estimated_lessons).sum()
    }
    
    pub fn print_summary(&self) {
        println!("=========================================");
        println!("MARISSELLE CURRICULUM SUMMARY");
        println!("=========================================");
        println!("Creator: {}", self.creator_name);
        println!("Version: {}", self.version);
        println!("Total Topics: {}", self.topics.len());
        println!("Total Estimated Lessons: {}", self.get_total_lessons());
        println!("");
        println!("Priority 10 (Highest): {}", self.get_topics_by_priority(10).len());
        println!("Priority 9: {}", self.get_topics_by_priority(9).len());
        println!("Priority 8: {}", self.get_topics_by_priority(8).len());
        println!("Priority 7: {}", self.get_topics_by_priority(7).len());
        println!("");
        println!("FINAL DIRECTIVE:");
        println!("{}", self.final_directive);
        println!("=========================================");
    }
}

impl Default for Curriculum {
    fn default() -> Self {
        Self::new()
    }
}
