// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import Button from "@mui/material/Button";
import ButtonGroup from "@mui/material/ButtonGroup";
import Box from "@mui/material/Box";
import { usePluginData } from "@docusaurus/useGlobalData";
import { useVersionContext } from "./versionContext";

import { VersionButtonProps, PluginData } from "../../types";

const VersionButton: React.FC<VersionButtonProps> = (props) => {
  const { versions } = usePluginData("sui-versions-plugin") as PluginData;

  const { state, setState } = useVersionContext();

  const handleClick = (
    event: React.MouseEvent<HTMLButtonElement>,
    index: number,
  ) => {
    const display =
      event.currentTarget.innerText.charAt(0).toUpperCase() +
      event.currentTarget.innerText.slice(1).toLowerCase();
    const versionObj = versions.find(
      (v) => v.display.toLowerCase() === display.toLowerCase(),
    );
    if (versionObj) {
      setState(versionObj);
    }
  };

  const options = versions.map((v) => v.display);

  return (
    <Box
      sx={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        "& > *": {
          m: 1,
        },
      }}
    >
      <ButtonGroup size="small" variant="text" aria-label="Network select">
        {options.map((network, index) => (
          <Button
            onClick={(e) => handleClick(e, index)}
            className={`${
              index === state.index
                ? "bg-sui-blue !text-sui-issue-dark"
                : "!text-black"
            }`}
            key={index}
          >
            {network}
          </Button>
        ))}
      </ButtonGroup>
    </Box>
  );
};

export default VersionButton;
