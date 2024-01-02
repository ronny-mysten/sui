// docs::https://docs.sui.io/guides/developer/app-examples/e2e-counter
// docs::#imports
import { TransactionBlock } from "@mysten/sui.js/transactions";
import { Button, Container } from "@radix-ui/themes";
// docs::#createCounterAbr
import {
  useSignAndExecuteTransactionBlock,
  useSuiClient,
} from "@mysten/dapp-kit";
// docs::#createCounterAbr-pause
import { useNetworkVariable } from "./networkConfig";
// docs::/#imports
// docs::#createCounter
// docs::#createCounterAbr-resume
export function CreateCounter({
  onCreated,
}: {
  onCreated: (id: string) => void;
}) {
  const client = useSuiClient();
  // docs::#createCounterAbr-pause
  const counterPackageId = useNetworkVariable("counterPackageId");
  // docs::#createCounterAbr-resume
  const { mutate: signAndExecute } = useSignAndExecuteTransactionBlock();

  return (
    // docs::#createCounterAbr-pause: <button />
    <Container>
      <Button
        size="3"
        onClick={() => {
          create();
        }}
      >
        Create Counter
      </Button>
    </Container>
    // docs::#createCounterAbr-resume
  );
  // docs::/#createCounterAbr}
  // docs::#create
  // docs::#createFull
  function create() {
    // docs::#createCounter-pause: // TODO
    const txb = new TransactionBlock();

    txb.moveCall({
      arguments: [],
      target: `${counterPackageId}::counter::create`,
    });

    signAndExecute(
      {
        transactionBlock: txb,
        options: {
          showEffects: true,
          showObjectChanges: true,
        },
      },
      {
        onSuccess: (tx) => {
          // docs::#create-pause
          client
            .waitForTransactionBlock({
              digest: tx.digest,
            })
            .then(() => {
              // docs::#create-resume
              const objectId = tx.effects?.created?.[0]?.reference?.objectId;

              if (objectId) {
                onCreated(objectId);
              }
              // docs::#create-pause
            });
          // docs::#create-resume
        },
      },
    );
    // docs::#createCounter-resume
  }
  // docs::/#createFull
  // docs::/#create
}
// docs::/#createCounter
