# Planning Projects

_Applied Self Management Using Lotus Agenda — Copyright 1992, Pages 4.2–4.5_

## Overview

Agenda's excellent date facilities make it an ideal tool for project planning. Project planning involves:

- Identifying the tasks that need to be carried out
- Deciding when they need to be done
- Identifying any dependencies between tasks
- Assigning the tasks to people

You would probably not use Agenda as a fully-blown project management system as there is a lot of more powerful, specialised software for this purpose. However, Agenda is ideal for small projects or for the planning phase of larger projects.

## Setting Up the Database

1. Create a new database with the name `projplan`.
2. Edit the `Initial Section` supplied by Agenda and change the name to `Task`.
3. Go to the Category Manager and add the following Categories:
   - `People` (standard category)
   - `Finish` (date category)
4. Return to the view and add columns for the following categories:
   - `People`
   - `When`
   - `Finish`
5. Change the column width of the `When` and `Finish` columns to be eight characters wide.

## Entering and Assigning Tasks

1. Think about a project that you may have to undertake in the near future.
2. Enter the tasks that are involved in this project as Items.
3. Assign the tasks to people by typing their names in the `People` column.
4. Show when the task must start by typing dates in the `When` column.
5. Type the date that the task must finish in the `Finish` column.

## Configuring the Tasks View

1. Select the View Properties dialogue box and change the name of the view to `Tasks`.
2. Create a new view of the database with the following properties:

   | Property  | Value                  |
   |-----------|------------------------|
   | View name | Task Assignments       |
   | View type | Standard               |
   | Sections  | Children of `People`   |

3. Create a third view of the database with the following properties:

   | Property  | Value    |
   |-----------|----------|
   | View name | Schedule |
   | View type | Datebook |
   | End category | End   |
   | Period    | Month    |
   | Interval  | Daily    |

## Setting Up Task Dependencies

1. Look carefully at the tasks you have entered.
2. Are there any tasks that are dependent on prior tasks being completed before they can commence? If not, invent some now and enter them as items.
3. Set up the task dependencies.
4. Change the View Properties to hide dependent Items.
5. Mark some of the tasks that are prerequisites for dependent tasks as `Done`.
6. Use Agenda's `Show View` to display the prerequisites for a dependent task.
