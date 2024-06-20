// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import semver from "semver";
import { useVersionContext } from "./versionContext";

import { VersionProps } from "../../types";

export const Version: React.FC<VersionProps> = ({
  version,
  featureID,
  children,
  versionsIncluded,
}) => {
  const orderedVersionsIncluded = versionsIncluded.sort(semver.compare);
  const totalVersions = orderedVersionsIncluded.length;

  const { state, setState } = useVersionContext();

  let show = false;
  // Index is the first occurrence of a version that is greater than or equal to this version
  const index = orderedVersionsIncluded.findLastIndex((v) =>
    semver.lte(v, version),
  );

  if (index >= 0) {
    if (
      (index === totalVersions - 1 && semver.lte(version, state.version)) ||
      (index < totalVersions - 1 &&
        semver.lte(orderedVersionsIncluded[index], state.version) &&
        semver.gt(orderedVersionsIncluded[index + 1], state.version))
    ) {
      show = true;
    }
  }

  return (
    <>
      {show && (
        <div
          className={`p-8 rounded-lg z-20 relative ${
            state.display === "Testnet"
              ? "bg-sui-ghost-white"
              : state.display === "Devnet"
              ? "bg-sui-ghost-white"
              : state.display === "Latest"
              ? "bg-sui-blue-light"
              : "bg-sui-gray-50"
          }`}
        >
          <p>
            This feature is available in {state.display} ({state.version})
          </p>
          <p>{version}</p>
          <p>{featureID}</p>
          <div>{children}</div>
        </div>
      )}
    </>
  );
};
