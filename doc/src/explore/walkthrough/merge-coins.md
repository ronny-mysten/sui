---
title: Merge Two Coins
---

In this example, you merge two coins to check the rebate you get. The previous example demonstrated the cost of gas paid with the total amount itemized in the `effects` object. That object includes a `storage_rebate` value, which is the amount you get back when you delete the object. For Coin objects, the reasonable way to delete them is to merge them with other coins. To carry forward the banknote example used previously, this is the equivalent of gathering smaller banknotes and exchanging them for larger denominations.

To merge, use the initial Coin whose address is already bound to `id` in your terminal. You need a coin to merge with, which in this example is the untouched Coin that is second in the list.

As you might expect, the end result of the following call is that the second object no longer exists and the first Coin object has a larger balance (sum of the two Coins) with the same ID.

```sh
# Bind the id of the second coin
id2="0x9f0afe6a6c3d72d281003b6f87ea84703113744c"
```

The method is `sui_mergeCoin` and the procedure is the same, but this time the `request_type` for the transaction is `ImmediateReturn`. The example continues by using the transaction digest and the `sui_getTransaction` to further see what happened to the execution. This example and those following might omit outputs already shown to focus attention on new responses.

```sh
# Prepare data for the sui_mergeCoins method
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_mergeCoins\", \"params\": [\"$address\", \"$id\", \"$id2\", \"$gas_id\", 1000]}"

# Fire the request
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json

# Get the tx_bytes from the result
tx_bytes="VHJhbnNhY3Rpb25EYXRhOjoAAgAAAAAAAAAAAAAAAAAAAAAAAAACAQAAAAAAAAAg61Q+9Nb1tpyLqvfx3DAp0d2s+c64rNfj1WaiqXiqd08DcGF5BGpvaW4BBwAAAAAAAAAAAAAAAAAAAAAAAAACA3N1aQNTVUkAAgEAkb57LAERJfdIwwiHLQDI8gDKvhUCAAAAAAAAACB9V3mu9pHYBp9Yq2Ms7Wd6ZLpCqzXGesc+TRFK1s8rnQEAnwr+amw9ctKBADtvh+qEcDETdEwBAAAAAAAAACCbQzldJ7gwtBazPWYA8UAfyLO3HxUedk82pi0Jvh9zWfwIv478w9s2IYqaMV/2x6C/DT0S6chaf0nA0GUmhWkNvVUv4XqDvS4CAAAAAAAAACDDQ9w3L/SkoSJ6hZD/iUXHxdqiYyei1KW1Fkwwx3gq0gEAAAAAAAAA6AMAAAAAAAA="

# Get the execute data
sui keytool sign --address "$address" --data "$tx_bytes"
# INFO sui::keytool: Address : 0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12
# INFO sui::keytool: Flag Base64: AA==
# INFO sui::keytool: Public Key Base64: R904IKMQHbULGI+8g3aKNndZHcXbO3FSRoZF3QspcnY=
# INFO sui::keytool: Signature : HIP5rNVmmvlCVq6nwvd2Ls7Uu9zUGyvw8T6+1N0LAmph123cX+bG2fsqyYcVpyT0imQZooSxNsac+89HsZVcCA==

# The public key and the scheme are the same
signature="HIP5rNVmmvlCVq6nwvd2Ls7Uu9zUGyvw8T6+1N0LAmph123cX+bG2fsqyYcVpyT0imQZooSxNsac+89HsZVcCA=="

# Prepare to execute the transaction
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_executeTransaction\", \"params\": [\"$tx_bytes\", \"$scheme\",\"$signature\",\"$pub_key\",\"ImmediateReturn\"]}"

# Request execution
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, the response resembles:

```JSON
{
    "jsonrpc": "2.0",
    "result": {
        "ImmediateReturn": {
            "tx_digest": "IuqUhuwnSp1LbJvWG8X9zbdl5LdiViJucAQb+KgYh2A="
        }
    },
    "id": 1
}
```

The `tx_digest` value is needed for the next call. Assign it to a variable:

```sh
tx_digest="IuqUhuwnSp1LbJvWG8X9zbdl5LdiViJucAQb+KgYh2A="

# Prepare the data for the sui_getTransaction
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_getTransaction\", \"params\": [\"$tx_digest\"]}"

# Send the request
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

A successful call here returns data similar to the following:

```JSON
{
    "jsonrpc": "2.0",
    "result": {
        "certificate": {
            "transactionDigest": "IuqUhuwnSp1LbJvWG8X9zbdl5LdiViJucAQb+KgYh2A=",
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
                            "function": "join",
                            "typeArguments": [
                                "0x2::sui::SUI"
                            ],
                            "arguments": [
                                "0x91be7b2c011125f748c308872d00c8f200cabe15",
                                "0x9f0afe6a6c3d72d281003b6f87ea84703113744c"
                            ]
                        }
                    }
                ],
                "sender": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12",
                "gasPayment": {
                    "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                    "version": 2,
                    "digest": "w0PcNy/0pKEieoWQ/4lFx8XaomMnotSltRZMMMd4KtI="
                },
                "gasBudget": 1000
            },
            "txSignature": "AByD+azVZpr5Qlaup8L3di7O1Lvc1Bsr8PE+vtTdCwJqYddt3F/mxtn7KsmHFack9IpkGaKEsTbGnPvPR7GVXAhH3TggoxAdtQsYj7yDdoo2d1kdxds7cVJGhkXdCylydg==",
            "authSignInfo": {
                "epoch": 0,
                "signature": "gkRkfIQWMESYbWtpiGLU45EbBvHTq8xufmp8TJkaYGYFunYWlGf1/6DMMisKrufm",
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
            "status": {
                "status": "success"
            },
            "gasUsed": {
                "computationCost": 528,
                "storageCost": 30,
                "storageRebate": 45
            },
            "transactionDigest": "IuqUhuwnSp1LbJvWG8X9zbdl5LdiViJucAQb+KgYh2A=",
            "mutated": [
                {
                    "owner": {
                        "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                    },
                    "reference": {
                        "objectId": "0x91be7b2c011125f748c308872d00c8f200cabe15",
                        "version": 3,
                        "digest": "y3YuwQfasEHD5lamA4su5+melRJWpP3s37uNX/u7F38="
                    }
                },
                {
                    "owner": {
                        "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                    },
                    "reference": {
                        "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                        "version": 3,
                        "digest": "zMc4fVHWOg4vVMa8l1JM4erSJKxz7I8BlluS5dWKoZE="
                    }
                }
            ],
            "deleted": [
                {
                    "objectId": "0x9f0afe6a6c3d72d281003b6f87ea84703113744c",
                    "version": 2,
                    "digest": "Y2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2M="
                }
            ],
            "gasObject": {
                "owner": {
                    "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                },
                "reference": {
                    "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
                    "version": 3,
                    "digest": "zMc4fVHWOg4vVMa8l1JM4erSJKxz7I8BlluS5dWKoZE="
                }
            },
            "events": [
                {
                    "deleteObject": {
                        "packageId": "0x0000000000000000000000000000000000000002",
                        "transactionModule": "pay",
                        "sender": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12",
                        "objectId": "0x9f0afe6a6c3d72d281003b6f87ea84703113744c"
                    }
                }
            ],
            "dependencies": [
                "PEGLG0dsS+4qPX6Qu77l2HauyB3eKsTfZoLExPp0GfU=",
                "3DEKooZaVHWiMrdjMb8e1h2kx2J+M9Yu5uJy2CHmgJ4="
            ]
        },
        "timestamp_ms": null,
        "parsed_data": null
    },
    "id": 1
}
```

As the response shows, the Coin object used for gas and the first Coin object are mutated, and the other Coin no longer exists. The result is `45` SUI as rebate and the Coin object assigned to `id` now has a balance of `19990001` SUI (you can check this with `sui client gas`).

With some straightforward examples completed from the previous exercises, you should be able to apply what you learned to similar calls like `sui_splitCoinEqual` and `sui_getRawObject` on your own. The exercises that follow interact with objects related to Move packages. 
