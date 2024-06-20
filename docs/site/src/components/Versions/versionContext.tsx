// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import React, { createContext, useState, useEffect, useContext } from "react";

const VersionContext = createContext();

const useVersionContext = () => {
  const context = useContext(VersionContext);
  if (!context) {
    throw new Error("useVersionContext must be used within a VersionProvider");
  }
  return context;
};

const VersionProvider = ({ children }) => {
  const [state, setState] = useState(() => {
    const savedState = localStorage.getItem("curSuiNetwork");
    return savedState
      ? JSON.parse(savedState)
      : {
          network: "main",
          display: "Latest",
          version: "1.28.0",
          index: 0,
        };
  });

  useEffect(() => {
    localStorage.setItem("curSuiNetwork", JSON.stringify(state));
  }, [state]);

  return (
    <VersionContext.Provider value={{ state, setState }}>
      {children}
    </VersionContext.Provider>
  );
};

export { VersionProvider, useVersionContext };
