// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import { usePluginData } from "@docusaurus/useGlobalData";
import { PluginData } from "../../types";

export const VersionTable: React.FC = () => {
  const { versions } = usePluginData("sui-versions-plugin") as PluginData;

  return (
    <table>
      <thead>
        <tr>
          <td>Network</td>
          <td>Version</td>
        </tr>
      </thead>
      <tbody>
        {versions.map((version, index) => (
          <tr key={index}>
            <td>{version.display}</td>
            <td>{version.version}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
};
