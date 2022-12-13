---
title: Mint Swords
---

Start by checking your module on the ledger with the `sui_getNormalizedMoveModule` method. This is a GET method, which is gas-less. You need the module ID, which is provided in the previous response, as well as the module name (`my_module` in this case).

```sh
module_id="0x9238689c6c14db84519686958643bf47de13e54e"
module_name="my_module"

data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_getNormalizedMoveModule\", \"params\": [\"$module_id\",\"$module_name\"]}"

curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, you get a response with the following shape:

```JSON
{
    "jsonrpc": "2.0",
    "result": {
        "file_format_version": 5,
        "address": "0x9238689c6c14db84519686958643bf47de13e54e",
        "name": "my_module",
        "friends": [],
        "structs": {
            "Forge": {
                "abilities": {
                    "abilities": [
                        "Key"
                    ]
                },
                "type_parameters": [],
                "fields": [
                    {
                        "name": "id",
                        "type_": {
                            "Struct": {
                                "address": "0x2",
                                "module": "object",
                                "name": "UID",
                                "type_arguments": []
                            }
                        }
                    },
                    {
                        "name": "swords_created",
                        "type_": "U64"
                    }
                ]
            },
            "Sword": {
                "abilities": {
                    "abilities": [
                        "Store",
                        "Key"
                    ]
                },
                "type_parameters": [],
                "fields": [
                    {
                        "name": "id",
                        "type_": {
                            "Struct": {
                                "address": "0x2",
                                "module": "object",
                                "name": "UID",
                                "type_arguments": []
                            }
                        }
                    },
                    {
                        "name": "magic",
                        "type_": "U64"
                    },
                    {
                        "name": "strength",
                        "type_": "U64"
                    }
                ]
            }
        },
        "exposed_functions": {
            "magic": {
                "visibility": "Public",
                "is_entry": false,
                "type_parameters": [],
                "parameters": [
                    {
                        "Reference": {
                            "Struct": {
                                "address": "0x9238689c6c14db84519686958643bf47de13e54e",
                                "module": "my_module",
                                "name": "Sword",
                                "type_arguments": []
                            }
                        }
                    }
                ],
                "return_": [
                    "U64"
                ]
            },
            "strength": {
                "visibility": "Public",
                "is_entry": false,
                "type_parameters": [],
                "parameters": [
                    {
                        "Reference": {
                            "Struct": {
                                "address": "0x9238689c6c14db84519686958643bf47de13e54e",
                                "module": "my_module",
                                "name": "Sword",
                                "type_arguments": []
                            }
                        }
                    }
                ],
                "return_": [
                    "U64"
                ]
            },
            "sword_create": {
                "visibility": "Public",
                "is_entry": true,
                "type_parameters": [],
                "parameters": [
                    {
                        "MutableReference": {
                            "Struct": {
                                "address": "0x9238689c6c14db84519686958643bf47de13e54e",
                                "module": "my_module",
                                "name": "Forge",
                                "type_arguments": []
                            }
                        }
                    },
                    "U64",
                    "U64",
                    "Address",
                    {
                        "MutableReference": {
                            "Struct": {
                                "address": "0x2",
                                "module": "tx_context",
                                "name": "TxContext",
                                "type_arguments": []
                            }
                        }
                    }
                ],
                "return_": []
            },
            "swords_created": {
                "visibility": "Public",
                "is_entry": false,
                "type_parameters": [],
                "parameters": [
                    {
                        "Reference": {
                            "Struct": {
                                "address": "0x9238689c6c14db84519686958643bf47de13e54e",
                                "module": "my_module",
                                "name": "Forge",
                                "type_arguments": []
                            }
                        }
                    }
                ],
                "return_": [
                    "U64"
                ]
            }
        }
    },
    "id": 1
}
```

By design, the response contains a lot of useful information. The `structs` object has `Forge` and `Sword` objects, simulating an actual forge that is used to create swords. The `exposed_functions` field has the functions used in following examples, like `sword_create`, `magic`, and `strength`. To give your forge a task, mint a magic katana for yourself, because the good things must be kept in-house. The method is `sui_moveCall` and needs more than a few parameters to work. Time to bind them to variables and prepare the data to have a clearer overview:

```sh
# Signer is our $address
# The package id
package_object_id="0x9238689c6c14db84519686958643bf47de13e54e"
module="my_module"
function="sword_create"
# The type arguments and arguments can be found in the previous result under the sword_create function
type_arguments="[]"
arguments="[\"0xeb8e4a532d09c596564f1c48b17f8ca4b339dbda\", \"455\", \"999\", \"$address\"]"

# Dump all variables into the data object
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_moveCall\", \"params\": [\"$address\", \"$package_object_id\", \"$module\", \"$function\", $type_arguments, $arguments, \"$gas_id\", 10000]}"

# Cross fingers and fire request
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

The requests should succeed. Before executing the transaction, let's take some time to recap the parameters of the request:

- `package_object_id` is the address/UID of the package you published unto Sui. You can see it in the `sui_getNormalizedMoveModule` response in many places, most notably as the value of `address`, the second key of the `result` JSON object.
- `module` is the module name; this is needed because a package might contain more than one module.
- `function` is the name of the function to run.
- `type_arguments` is being given by the function itself. Checking the module with the `sui_getNormalizedMoveModule` method showed that under the `sword_create` JSON object, the `type_paramaters` key has value `[]`. Consequently, `[]` is the value passed with this call.
- `arguments` is another reference to the values from the response to the `sui_getNormalizedMoveModule` call. The `sword_create` function has a few parameters. The JSON-Move subset of JSON requires that arrays are homogenous. This `arguments` array contains only strings (as is usual for most arrays in JSON RPC requests). The first argument for the function is a `mutable reference to the Forge object`, this is encoded as `id of the object` so we just put the id of the Forge that has been created previously. `Magic` and `Strength`, as seen in the parameters field (inside the `sui_getNormalizedMoveModule` result) are both `U64` (unsigned integer 64 bits), but the request uses `\"455\"` and `\"999\"` as strings to create an homogenous array, relying on the Sui Node to translate into proper types. 
- `TxContext` is the last required parameter. The quick-witted have already noticed that the parameter is omitted from the call, though. This is intentional as Sui fills in the value when it runs the function. This holds for any function that has `Mutable Reference TxContext` (or `&mut TxContext` in Move source code).

The last two arguments are the usual arguments for gas deduction, `$gas_id` being the Coin object ID and `10000` being the gas budget value.

Continuing, bind the `tx_bytes` value and execute the transaction.

```sh
# Get txBytes from previous result
tx_bytes="VHJhbnNhY3Rpb25EYXRhOjoAApI4aJxsFNuEUZaGlYZDv0feE+VOAQAAAAAAAAAg+pryAo+gALTc0mDEz9wJ3XFmmJ4IMz131namNQmzj4MJbXlfbW9kdWxlDHN3b3JkX2NyZWF0ZQAEAQDrjkpTLQnFllZPHEixf4yksznb2gEAAAAAAAAAIIq8/a2erR0ImDILYDhh3kBL5EXVUkFo5TgCvFzOBhBqAAjHAQAAAAAAAAAI5wMAAAAAAAAAFPwIv478w9s2IYqaMV/2x6C/DT0S/Ai/jvzD2zYhipoxX/bHoL8NPRL6Ecj//u2x1EZIs53WLaU5X9U2ygIAAAAAAAAAIAM0VDf2JZIbgYIbHcUCdS/RZUWF7uxa1hVW98o9wzlcAQAAAAAAAAAQJwAAAAAAAA=="

# Get the signature
sui keytool sign --address "$address" --data "$tx_bytes"

signature="BXESHr1zl9p/oAegLcAwWQq/F6ljHg+a8N2+GOphlaW44GKJAvbGV6Zjs4E0Di0ZRb7O/9HrCzXY17WJU+7sDg=="

data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_executeTransaction\", \"params\": [\"$tx_bytes\", \"$scheme\",\"$signature\",\"$pub_key\",\"WaitForLocalExecution\"]}"

curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, you get a response representing ownership of your new katana:

```JSON
// ...
    "created": [
        {
            "owner": {
                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
            },
            "reference": {
                "objectId": "0x36e8443ea817223639ef6bdd5400d2bfa1b673f2",
                "version": 1,
                "digest": "To13qwyuk+zgFskF5aN8LOt4w7cHEosqZ89ZO354J80="
            }
        }
    ]
//...
```

You now have a sword with imba stats. In the next example, you send your sword to a friend (or generally speaking, another address on the ledger).