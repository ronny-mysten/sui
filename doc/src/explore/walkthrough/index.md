---
title: JSON RPC Examples
---

Welcome to this walkthrough of the Sui JSON Remote Procedure Call (RPC) protocol. Using the following series of examples, you interact with the Sui network to create and manipulate objects on the blockchain using the Sui Client CLI.

## Prerequisites

What you need:
* Sui Client CLI (installed when you install Sui). To check if you have Sui installed, type `which sui` in a command-line shell (terminal) window. If the terminal does not echo a path, you need to [install Sui](/devnet/build/install).
* An active address, preferably without any objects. You create an address when you first connect to Devnet with the `sui client` command. If you need a new address, use the `sui client new-address ed25519` command to create one with an `ed25519` key scheme.   
* cURL is used in the examples. You can modify these calls to the language of your choice. If you need help installing, refer to the [Install Sui to Build](/build/install) topic.
* The examples use Git to download code from the internet. If you need help installing, refer to the [Install Sui to Build](/build/install) topic.

## Setup

**Note:** When reading about Sui, you often see Sui capitalized as both "Sui" and "SUI". Sui refers to the network, and SUI, all capital letters, refers to the token.

Open a terminal window. If you are following along using cURL, use the same terminal window for all the commands in these examples.

To begin, make sure your address exists and store its value to a variable to limit the amount of typing required in subsequent commands: 

```sh
 sui client active-address
 # 0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12
 address="0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
```

**Note:** Sample values are provided throughout these examples to demonstrate what they should look like. Make sure to change them to the specific values for your environment.  

If the sui command does not return an address, [connect to Sui Devnet](https://docs.sui.io/build/devnet) to get one. 

To complete the following calls, you need to add SUI to your address. If using Devnet, join our [discord channel](https://discord.gg/sui) if you haven't already. After you are verified, type `!faucet <address>` in the the [devnet-faucet](https://discord.com/channels/916379725201563759/971488439931392130) channel, where `<address>` is the value you retrieve from the `sui client active-address` command.

If using a local installation, then add SUI to your address.

To complete setup, bind the devnet rpc server URL to a variable:

```sh
rpc="https://fullnode.devnet.sui.io:443"
```

Now, you are ready to make your first call and see the SUI you own.
