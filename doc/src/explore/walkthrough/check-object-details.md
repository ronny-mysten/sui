---
title: Check the details of an Object
---

In the previous example, you retrieved the objects your address owns. In this example, you use the `sui_getObject` method to check the <a href="https://docs.sui.io/sui-jsonrpc#sui_getObject">`objectId`</a> for one of those coins and its details. Enter the following commands in your terminal, replacing the given ID with one of the coin object IDs at your address.

```sh
# Set the id as a variable
id="0x91be7b2c011125f748c308872d00c8f200cabe15"
# Create the JSON
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_getObject\", \"params\": [\"$id\"]}"
# Fire the request and save the result in a tmp file
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

After entering the last command, open the result.json file it created in an editor like Visual Studio Code. Use the editor to format the JSON, if necessary. If successful, your JSON resembles the following:

```JSON
{
    "jsonrpc":"2.0",
    "result":{
        "status":"Exists",
        "details":{
            "data":{
                "dataType":"moveObject",
                "type":"0x2::coin::Coin<0x2::sui::SUI>",
                "has_public_transfer":true,
                "fields":{
                    "balance":10000000,
                    "id":{"id":"0x91be7b2c011125f748c308872d00c8f200cabe15"}
                    }
                },
        "owner":{"AddressOwner":"0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"},
        "previousTransaction":"8Dpc1+03kCviQRrB4hMUDT1z3bF+ZhlHtkG3RXHKuBg=",
        "storageRebate":15,
        "reference":{"objectId":"0x91be7b2c011125f748c308872d00c8f200cabe15",
        "version":1,
        "digest":"Bu5RPYiVWDDX4IshFqB8vsUdzBptTkhNK/Hm/b06wEA="}}
    },
    "id":1
}
```

The same parameters exist for this call as were available with [`sui_getObjectsOwnedByAddress`](/sui-jsonrpc#sui_getObjectsOwnedByAddress) (`digest`, `previousTransaction`, `owner`, `version`, and so on) but are in a slightly different shape with additional data. The `result` object, for example, contains a `details.data.fields.balance` parameter showing a value of `10000000`. This means that this SUI Coin object's value is 10,000,000 SUI. The faucet provided you with five such objects. If you check each one, you discover that their balance is the same.

As mentioned, when you want to acquire another object that is priced in SUI, you need to send a Coin object with the exact balance as its price. You also must provide a Coin object to pay gas for the transaction.

The splitting of Coins to facilitate transactions typically does not require user input or knowledge, so the process occurs quietly. In the next example, you split the coins manually to understand the process.