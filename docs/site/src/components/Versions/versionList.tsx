// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import { usePluginData } from "@docusaurus/useGlobalData";
import { PluginData } from "../../types";

export const VersionList: React.FC = () => {
  const { versions } = usePluginData("sui-versions-plugin") as PluginData;

  return (
    <div>
      {versions.map((version, index) => (
        <p
          key={index}
          className="text-sui-gray-11 py-2 mb-0 border-0 border-b-1 border-solid border-sui-line"
        >
          <span className="inline-block w-1/2">{version.display}</span>
          <span className="inline-block text-end font-bold">
            {version.version}
          </span>
        </p>
      ))}
    </div>
  );
};
