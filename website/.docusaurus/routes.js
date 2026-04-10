import React from 'react';
import ComponentCreator from '@docusaurus/ComponentCreator';

export default [
  {
    path: '/mcpzip/docs',
    component: ComponentCreator('/mcpzip/docs', 'a78'),
    routes: [
      {
        path: '/mcpzip/docs',
        component: ComponentCreator('/mcpzip/docs', 'e2b'),
        routes: [
          {
            path: '/mcpzip/docs',
            component: ComponentCreator('/mcpzip/docs', '583'),
            routes: [
              {
                path: '/mcpzip/docs/',
                component: ComponentCreator('/mcpzip/docs/', 'aa0'),
                exact: true,
                sidebar: "docsSidebar"
              },
              {
                path: '/mcpzip/docs/architecture',
                component: ComponentCreator('/mcpzip/docs/architecture', 'e19'),
                exact: true,
                sidebar: "docsSidebar"
              },
              {
                path: '/mcpzip/docs/configuration',
                component: ComponentCreator('/mcpzip/docs/configuration', '149'),
                exact: true,
                sidebar: "docsSidebar"
              },
              {
                path: '/mcpzip/docs/contributing',
                component: ComponentCreator('/mcpzip/docs/contributing', '744'),
                exact: true,
                sidebar: "docsSidebar"
              },
              {
                path: '/mcpzip/docs/getting-started',
                component: ComponentCreator('/mcpzip/docs/getting-started', '753'),
                exact: true,
                sidebar: "docsSidebar"
              },
              {
                path: '/mcpzip/docs/search',
                component: ComponentCreator('/mcpzip/docs/search', '963'),
                exact: true,
                sidebar: "docsSidebar"
              },
              {
                path: '/mcpzip/docs/transports',
                component: ComponentCreator('/mcpzip/docs/transports', 'bac'),
                exact: true,
                sidebar: "docsSidebar"
              }
            ]
          }
        ]
      }
    ]
  },
  {
    path: '/mcpzip/',
    component: ComponentCreator('/mcpzip/', '39c'),
    exact: true
  },
  {
    path: '*',
    component: ComponentCreator('*'),
  },
];
