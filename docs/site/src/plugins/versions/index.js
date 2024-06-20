// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// This plugin gets the versions for
// main, Mainnet, Testnet, Devnet
// { network: string, display: String, version: string }

import axios from "axios";
import semver from "semver";

const networks = ["main", "devnet", "testnet", "mainnet"];
const tomlUrl =
  "https://raw.githubusercontent.com/MystenLabs/sui/network/Cargo.toml";

const getVersion = async (network) => {
  const url = tomlUrl.replace("network", network);

  const ver = await axios
    .get(url)
    .then((response) => {
      console.log(url);
      return response.data.match(
        /\[workspace\.package\][\s\S]*?version\s*=\s*"(\d+\.\d+\.?\d?)"/,
      )[1];
    })
    .catch((error) => {
      console.error(
        "Error fetching the Cargo.toml file for versioning:",
        error,
      );
      return "Err";
    });

  return ver;
};

const versionPlugin = (context, options) => {
  return {
    name: "sui-versions-plugin",

    async loadContent() {
      let versions = [];
      let index = 0;
      for (const network of networks) {
        const version = await getVersion(network);
        const display =
          network === "main"
            ? "Latest"
            : network.charAt(0).toUpperCase() + network.slice(1);
        versions.push({ network, display, version, index });
        index++;
      }
      versions = versions.sort((a, b) => semver.compare(b.version, a.version));
      return { versions };
    },
    // This function exposes the loaded content to `globalData`
    async contentLoaded({ content, actions }) {
      const { setGlobalData } = actions;
      setGlobalData(content);
    },
  };
};

module.exports = versionPlugin;
