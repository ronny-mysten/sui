---
title: Transfer Objects
---

The last example involves transferring the magic katana created previously to another address. The method to do so is `sui_transferObject`.

The methods are fairly straightforward and similar to `sui_splitCoin` used in a previous example. You need a recipient address, someone else's or a second address of your own.


```sh
# Address who will receive the stuff
friend_address="0x788a511738ad4ab1d7f769b49076d9c7b272826c"

# The id of the Sword NFT
sword_id="0x36e8443ea817223639ef6bdd5400d2bfa1b673f2"

# Prepare the data params: [<signer>, <object_id>, <gas>, <gas_budget>, <recipient>]
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_transferObject\", \"params\": [\"$address\", \"$sword_id\", \"$gas_id\", 10000, \"$friend_address\"]}"

# Send our transaction to be validated
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json

# Get the tx_bytes
tx_bytes="VHJhbnNhY3Rpb25EYXRhOjoAAHiKURc4rUqx1/dptJB22ceycoJsNuhEPqgXIjY572vdVADSv6G2c/IBAAAAAAAAACBOjXerDK6T7OAWyQXlo3ws63jDtwcSiypnz1k7fngnzfwIv478w9s2IYqaMV/2x6C/DT0S+hHI//7tsdRGSLOd1i2lOV/VNsoDAAAAAAAAACDh08xwMUIlEXznVO/kW9uVptN+cK351OvieMgvhlB3vgEAAAAAAAAAECcAAAAAAAA="

# Get the signature
sui keytool sign --address "$address" --data "$tx_bytes"

signature="LXDnYcjFQU6boMyp+ZBbeYr1dSjcwjnXu9mEtu/yDApf/edHyNzUT4xGW6umDnc7IW539qP04wihrkyjKVF1CA=="
pub_key="R904IKMQHbULGI+8g3aKNndZHcXbO3FSRoZF3QspcnY="
scheme="ED25519"

# Data for the execution
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_executeTransaction\", \"params\": [\"$tx_bytes\", \"$scheme\",\"$signature\",\"$pub_key\",\"WaitForLocalExecution\"]}"

# Say goodbye to your sword:
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, the response confirms transfer:

```JSON
//...
"mutated": [
    {
        "owner": {
            "AddressOwner": "0x788a511738ad4ab1d7f769b49076d9c7b272826c"
        },
        "reference": {
            "objectId": "0x36e8443ea817223639ef6bdd5400d2bfa1b673f2",
            "version": 2,
            "digest": "oNqMlWH+Eslz+7ZLVtEKddWw1UlPmPj0tQAwWRoRzYg="
        }
    }, //...
]
//...
```

The `AddressOwner` is your `$friend_address` for the sword (`object_id` same as `$sword_id`). Also, the sword's version got bumped up to 2, meaning it has experienced two transactions: first transaction being its forging (actually, it's the transfer to our address after creation, if you check the source code for the [`sword_create`](https://github.com/MystenLabs/sui/blob/main/sui_programmability/examples/move_tutorial/sources/my_module.move) function), and the second transaction is the transfer you just initiated.