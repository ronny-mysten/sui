// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import VersionButton from "./versionButton";
import { useVersionContext } from "./versionContext";

import { VersionedProps } from "../../types";

export const Versioned: React.FC<VersionedProps> = ({ children }) => {
  const { state, setState } = useVersionContext();

  const versionsIncluded = React.Children.map(children, (child) =>
    React.isValidElement(child) ? child.props.version : null,
  ).filter((version) => version !== null) as string[];

  const renderChildren = () => {
    return React.Children.map(children, (child) => {
      return React.isValidElement(child)
        ? React.cloneElement(child, {
            versionsIncluded,
          })
        : child;
    });
  };

  return (
    <div className="p-1 rounded-lg bg-sui-gray-50 dark:bg-sui-ghost-dark border border-solid border-sui-steel-dark min-h-32">
      <div className="flex justify-between items-center px-4">
        <p className="font-bold pt-4">Versioned content</p>
        <VersionButton />
        <p className="text-sm">
          Viewing {state.display} (
          <span className="font-bold">{state.version}</span>)
        </p>
      </div>
      <div className="z-10 px-8 -mb-12 mt-2 ">
        This content is versioned and the features or functions it documents are
        not yet available in the chosen network. Use the tabs above to switch
        the targeted network.
      </div>
      {renderChildren()}
    </div>
  );
};
