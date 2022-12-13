---
title: Split a SUI Coin object
---

In this example, you use the <a href="/sui-jsonrpc#sui_splitCoin">`sui_splitCoin`</a> method to split one of your Coin objects into two objects: one with 9,999 SUI and another with 9,990,001 SUI. This is a transaction, as such it will require payment of gas. In this instance, you use another Coin object to pay for the gas (the same object will be used consistently for this purpose for future exercises). For demonstration, this example uses the object with ID `0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e`. 

Bind this id to a variable in your terminal (replacing the example ID with one of your object IDs, of course):

```sh
gas_id="0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e"
```
> **NOTE:** This same Coin object is used for future gas payments in following exercises.

The parameters for the request payload are `signer`, `coin_object_id`, `split_amounts`, `gas`, and `gas_budget`, where `gas` is the Coin object to subtract gas from. For now, this example sets a large budget for the `gas_budget` parameter to make sure the transaction succeeds. Subsequent examples show how you can check the gas price reference to fine tune your transactions.

This is the data to send with your curl request:

```sh
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_splitCoin\", \"params\": [\"$address\", \"$id\", [9999], \"$gas_id\", 100000]}"
```
With `data` set, run the curl command:
```sh
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, your response resembles the following:

```JSON
{
    "jsonrpc": "2.0",
    "result": {
        "txBytes": "VHJhbnNhY3Rpb25EYXRhOjoAAgAAAAAAAAAAAAAAAAAAAAAAAAACAQAAAAAAAAAg61Q+9Nb1tpyLqvfx3DAp0d2s+c64rNfj1WaiqXiqd08DcGF5CXNwbGl0X3ZlYwEHAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQCRvnssAREl90jDCIctAMjyAMq+FQEAAAAAAAAAIGbrG8PNNFpOPr6ZHovCUA/hCYkBH/D75nSEKJB8F9DdAAkBDycAAAAAAAD8CL+O/MPbNiGKmjFf9segvw09EunIWn9JwNBlJoVpDb1VL+F6g70uAQAAAAAAAAAgBptpMcUxbtjN0D9CGvtqSwS9odmEfW9pho+aIF/MiA4BAAAAAAAAAKCGAQAAAAAA",
        "gas": {
            "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
            "version": 1,
            "digest": "BptpMcUxbtjN0D9CGvtqSwS9odmEfW9pho+aIF/MiA4="
        },
        "inputObjects": [
            {
                "ImmOrOwnedMoveObject": {
                    "objectId": "0x91be7b2c011125f748c308872d00c8f200cabe15",
                    "version": 1,
                    "digest": "Zusbw800Wk4+vpkei8JQD+EJiQEf8PvmdIQokHwX0N0="
                }
            },
            {
                "MovePackage": "0x0000000000000000000000000000000000000002"
            },
            {
                "ImmOrOwnedMoveObject": {
                    "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                    "version": 1,
                    "digest": "BptpMcUxbtjN0D9CGvtqSwS9odmEfW9pho+aIF/MiA4="
                }
            }
        ]
    },
    "id": 1
}
```

This response might not quite be what you expected. The main point of this request is to get the transaction bytes that the virtual machine understands and can execute. This means our initial intent to split the Coin object is not yet realized. You now have the `tx_bytes` value, however, which you can supply to the [`sui_executeTransaction`](/sui-jsonrpc#sui_executeTransaction) method to perform the actual splitting. This is the standard procedure when using the RPC directly for any type of transaction that mutates objects (not a GET request).

A review of the parameters needed for the `sui_executeTransaction` method shows some parameters that have not been introduced yet: `sig_scheme`, `signature`, `pub_key`, and `request_type`. Use the `sui keytool` command from the sui CLI. The exact command is `sui keytool sign --address <owner_address> --data <tx_bytes>`.

```sh
# For clarity, bind the transactions bytes (txBytes) from the previous response to a variable
tx_bytes="VHJhbnNhY3Rpb25EYXRhOjoAAgAAAAAAAAAAAAAAAAAAAAAAAAACAQAAAAAAAAAg61Q+9Nb1tpyLqvfx3DAp0d2s+c64rNfj1WaiqXiqd08DcGF5CXNwbGl0X3ZlYwEHAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQCRvnssAREl90jDCIctAMjyAMq+FQEAAAAAAAAAIGbrG8PNNFpOPr6ZHovCUA/hCYkBH/D75nSEKJB8F9DdAAkBDycAAAAAAAD8CL+O/MPbNiGKmjFf9segvw09EunIWn9JwNBlJoVpDb1VL+F6g70uAQAAAAAAAAAgBptpMcUxbtjN0D9CGvtqSwS9odmEfW9pho+aIF/MiA4BAAAAAAAAAKCGAQAAAAAA"

sui keytool sign --address "$address" --data "$tx_bytes"
# INFO sui::keytool: Address : 0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12
# INFO sui::keytool: Flag Base64: AA==
# INFO sui::keytool: Public Key Base64: R904IKMQHbULGI+8g3aKNndZHcXbO3FSRoZF3QspcnY=
# INFO sui::keytool: Signature : 6LARTREvuT81FxtJPUI9XqoWLlNUTc3yg3yG6i3va3XhDi9WlIgu1ZXmC88VUD3fMeW5EUOh81DU8bLILXoTBw==
```
The `sig_scheme` is the digital signature scheme you chose at address creation, which is `ED25519` by default.

The `request_type` value can be one of `ImmediateReturn`, `WaitForTxCert`, `WaitForEffectsCert`, `WaitForLocalExecution`. The value provided sets how much information is returned by the response. Although everything on the Sui network happens very fast, you could submit a value of `ImmediateReturn` if you are programmatically in a hurry to favor response time over information returned. For this exercise, set the value to `WaitForLocalExecution` to return the complete set of information. 

With all the necessary parameter values known, it's time to execute the transaction:

```sh
# Bind data to variables
signature="6LARTREvuT81FxtJPUI9XqoWLlNUTc3yg3yG6i3va3XhDi9WlIgu1ZXmC88VUD3fMeW5EUOh81DU8bLILXoTBw=="
scheme="ED25519"
pub_key="R904IKMQHbULGI+8g3aKNndZHcXbO3FSRoZF3QspcnY="

# Form the payload
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_executeTransaction\", \"params\": [\"$tx_bytes\", \"$scheme\",\"$signature\",\"$pub_key\",\"WaitForLocalExecution\"]}"

# Fire the request
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, the response is long and verbose:

```JSON
{
    "jsonrpc": "2.0",
    "result": {
        "EffectsCert": {
            "certificate": {
                "transactionDigest": "PEGLG0dsS+4qPX6Qu77l2HauyB3eKsTfZoLExPp0GfU=",
                "data": {
                    "transactions": [
                        {
                            "Call": {
                                "package": {
                                    "objectId": "0x0000000000000000000000000000000000000002",
                                    "version": 1,
                                    "digest": "61Q+9Nb1tpyLqvfx3DAp0d2s+c64rNfj1WaiqXiqd08="
                                },
                                "module": "pay",
                                "function": "split_vec",
                                "typeArguments": [
                                    "0x2::sui::SUI"
                                ],
                                "arguments": [
                                    "0x91be7b2c011125f748c308872d00c8f200cabe15",
                                    [
                                        9999
                                    ]
                                ]
                            }
                        }
                    ],
                    "sender": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12",
                    "gasPayment": {
                        "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                        "version": 1,
                        "digest": "BptpMcUxbtjN0D9CGvtqSwS9odmEfW9pho+aIF/MiA4="
                    },
                    "gasBudget": 100000
                },
                "txSignature": "AOiwEU0RL7k/NRcbST1CPV6qFi5TVE3N8oN8huot72t14Q4vVpSILtWV5gvPFVA93zHluRFDofNQ1PGyyC16EwdH3TggoxAdtQsYj7yDdoo2d1kdxds7cVJGhkXdCylydg==",
                "authSignInfo": {
                    "epoch": 0,
                    "signature": "kU/ZpPuirQw+UzEE2FjsAdQD70SMEYF67S0DZR2yy5Ns673/ws/RbOM8uSm3wkhQ",
                    "signers_map": [
                        58,
                        48,
                        0,
                        0,
                        1,
                        0,
                        0,
                        0,
                        0,
                        0,
                        2,
                        0,
                        16,
                        0,
                        0,
                        0,
                        0,
                        0,
                        2,
                        0,
                        3,
                        0
                    ]
                }
            },
            "effects": {
                "transactionEffectsDigest": "asC3MKdc8nDhlkeMthWiSZdRRNOq4Gi0bZijA+Nlu3Y=",
                "effects": {
                    "status": {
                        "status": "success"
                    },
                    "gasUsed": {
                        "computationCost": 557,
                        "storageCost": 45,
                        "storageRebate": 30
                    },
                    "transactionDigest": "PEGLG0dsS+4qPX6Qu77l2HauyB3eKsTfZoLExPp0GfU=",
                    "created": [
                        {
                            "owner": {
                                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                            },
                            "reference": {
                                "objectId": "0x0f848b589fe7727a628359c26bf880c9efb1de3b",
                                "version": 1,
                                "digest": "FmBwMRf+r1QlTf3XXBYizElY++W4w19tjeSLmTDpgbI="
                            }
                        }
                    ],
                    "mutated": [
                        {
                            "owner": {
                                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                            },
                            "reference": {
                                "objectId": "0x91be7b2c011125f748c308872d00c8f200cabe15",
                                "version": 2,
                                "digest": "fVd5rvaR2AafWKtjLO1nemS6Qqs1xnrHPk0RStbPK50="
                            }
                        },
                        {
                            "owner": {
                                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                            },
                            "reference": {
                                "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                                "version": 2,
                                "digest": "w0PcNy/0pKEieoWQ/4lFx8XaomMnotSltRZMMMd4KtI="
                            }
                        }
                    ],
                    "gasObject": {
                        "owner": {
                            "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                        },
                        "reference": {
                            "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                            "version": 2,
                            "digest": "w0PcNy/0pKEieoWQ/4lFx8XaomMnotSltRZMMMd4KtI="
                        }
                    },
                    "events": [
                        {
                            "newObject": {
                                "packageId": "0x0000000000000000000000000000000000000002",
                                "transactionModule": "pay",
                                "sender": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12",
                                "recipient": {
                                    "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                                },
                                "objectId": "0x0f848b589fe7727a628359c26bf880c9efb1de3b"
                            }
                        }
                    ],
                    "dependencies": [
                        "B2015vW4kYmFXLsBw/njGWUOQHdmszg8jsdygrUFTPw=",
                        "lBnZ6OXExGfd0C5EzCLlR2gXAT/aTn0EnOvwF5KJEOU="
                    ]
                },
                "authSignInfo": {
                    "epoch": 0,
                    "signature": "gp4NdTfonxPet2v+v2gdEmBhLs7Ii7zZ6UmSjKvuv8h7KxFtWZaMqVamk9QXiL9f",
                    "signers_map": [
                        58,
                        48,
                        0,
                        0,
                        1,
                        0,
                        0,
                        0,
                        0,
                        0,
                        2,
                        0,
                        16,
                        0,
                        0,
                        0,
                        0,
                        0,
                        1,
                        0,
                        3,
                        0
                    ]
                }
            },
            "confirmed_local_execution": true
        }
    },
    "id": 1
}
```

The `effects` object shows the transaction was successful. There is now a mutated object with the ID of the initial coin. The `version` value got bumped up to `2` to signal the object has experienced two transactions: this one and the creation of the object. There is also a newly created object with ID `0x0f848b589fe7727a628359c26bf880c9efb1de3b==`.

If you check your new object with `sui_getObject`, you should get a response similar to:

```JSON
"result": {
        "status": "Exists",
        "details": {
            "data": {
                "dataType": "moveObject",
                "type": "0x2::coin::Coin<0x2::sui::SUI>",
                "has_public_transfer": true,
                "fields": {
                    "balance": 9999,
                    "id": {
                        "id": "0x0f848b589fe7727a628359c26bf880c9efb1de3b=="
                    }
                }
            }, // ...
        } // ...
}            
```
The response confirms the existence of a Coin object with 9,999 worth of SUI. If you check all your owned objects (first example), you can see the other two coins that changed, one with balance `9990001` SUI and the other with `9999428` SUI. The latter is the one that was used to pay for the gas.

The next example explores the other `request_type` values and shows how to subscribe to events so you know what's happening with your transactions.