---
title: Interlude
---

Now might be a good time to head out to the lobby and grab yourself a snack. When you return, start the next examples that involve publishing and using a Move module on the blockchain. In the interest of time, these examples use a sample module rather than creating one from scratch. To learn more about Move and how to write modules suitable for Sui, check out [examples.sui.io](http://examples.sui.io/). 

To get the necessary Move package, clone the Sui repo. The module to explore is `move_tutorial` located in the `sui/sui_programmability/examples` directory. 

```sh
# clone the repo in the work or home directory
git clone https://github.com/MystenLabs/sui.git
```

You must first build modules before you can publish them. If you try to publish as is, you get an error. To get ahead of that situation, alter the `Move.toml` file inside `sui/sui_programmability/examples/move_tutorial/`. Change the following line:

```
[dependencies]
Sui = { local = "../../../crates/sui-framework" }
```

to

```
[dependencies]
Sui = { git = "https://github.com/MystenLabs/sui.git", subdir = "crates/sui-framework", rev = "devnet" }
```

Also, at the end of the `Move.toml` file add the following line:

```
sui =  "0000000000000000000000000000000000000002"
```

Now the module is ready to be built. Inside the `sui/sui_programmability/examples/move_tutorial/` directory, run:

```sh
# Inside sui/sui_programmability/examples/move_tutorial/
sui move build --dump-bytecode-as-base64
# ["oRzrCwUAAAAKAQAIAggQAxgpBEEEBUUsB3F9CO4BKAqWAhIMqAJyDZoDBgAAAQEBAgEDAAQIAAAFDAADBgIAAQ0EAAAHAAEAAAgCAwAACQIDAAAKBAEAAAsFAwABDgAHAAMPCAkAAgIKAQEIBwYHCwEHCAIAAQYIAQEDBQcIAAMDBQcIAgEGCAABCAABCAMBBggCAQUCCQAFAQgBCW15X21vZHVsZQZvYmplY3QIdHJhbnNmZXIKdHhfY29udGV4dAVGb3JnZQVTd29yZAlUeENvbnRleHQEaW5pdAVtYWdpYwhzdHJlbmd0aAxzd29yZF9jcmVhdGUOc3dvcmRzX2NyZWF0ZWQCaWQDVUlEA25ldwZzZW5kZXIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAAICDAgDCwMBAgMMCAMIAwkDAAAAAAYLCgARBQYAAAAAAAAAABIADAELAQsALhEGOAACAQEAAAEECwAQABQCAgEAAAEECwAQARQCAwEEAAsSCwQRBQsBCwISAQwFCwULAzgBCgAQAhQGAQAAAAAAAAAWCwAPAhUCBAEAAAEECwAQAhQCAQEBAgABAA=="]
```

If you receive an error, you might need to update your Sui client. Follow the instructions the Sui client displays in your terminal to update.